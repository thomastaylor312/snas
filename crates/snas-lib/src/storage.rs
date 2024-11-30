use std::{collections::HashMap, sync::Arc};

use anyhow::Context;
use async_nats::jetstream::kv::{Entry, Operation, Store};
use futures::{StreamExt, TryStreamExt};
use tokio::{sync::RwLock, task::AbortHandle};
use tracing::{debug, error, info, instrument, trace, Instrument};

use crate::types::UserInfo;

/// A read cache for the credentials store along with methods for adding, updating, and deleting
/// credentials.
pub struct CredStore {
    store: Store,
    cache: Arc<RwLock<HashMap<String, UserInfo>>>,
    // REMINDER: If we need to implement clone, then this should be wrapped in a struct that
    // implements drop rather than implementing drop on this struct.
    update_handle: AbortHandle,
}

impl Drop for CredStore {
    fn drop(&mut self) {
        self.update_handle.abort()
    }
}

impl CredStore {
    #[instrument(level = "info", skip_all)]
    pub async fn new(store: Store) -> anyhow::Result<Self> {
        let cache = Arc::new(RwLock::new(HashMap::new()));
        let cache_clone = cache.clone();
        let store_clone = store.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();
        let update_handle = tokio::spawn(
            async move {
                // Start the watcher first so we can catch any updates that happen after we query all data
                let mut watcher = match store_clone.watch_all().await {
                    Ok(watcher) => watcher,
                    Err(e) => {
                        // If we can't send, that is fatal and we should panic
                        tx.send(Err(anyhow::Error::from(e)))
                            .expect("Failed to send error when setting up watcher");
                        return;
                    }
                };
                info!("Fetching initial data for local cache");
                let data = match initial_data_fetch(&store_clone).await {
                    Ok(d) => d,
                    Err(e) => {
                        // If we can't send, that is fatal and we should panic
                        tx.send(Err(e))
                            .expect("Failed to send error when setting up watcher");
                        return;
                    }
                };
                {
                    let mut lock = cache_clone.write().await;
                    *lock = data;
                }

                debug!("Data initialization complete, starting watch");
                tx.send(Ok(()))
                    .expect("Unable to send complete signal when setting up watcher");

                while let Some(res) = watcher.next().await {
                    match res {
                        Ok(entry) => handle_entry(entry, &cache_clone).await,
                        Err(err) => {
                            error!(%err, "Error when attempting to receive next value");
                        }
                    }
                }
            }
            .instrument(tracing::info_span!("cache_updater")),
        );

        rx.await??;
        info!("Cred store initialization complete");

        Ok(Self {
            store,
            cache,
            update_handle: update_handle.abort_handle(),
        })
    }

    /// Checks if the username exists. This will always check the cache first and then the store to
    /// ensure that operations such as creating a new user are atomic.
    #[instrument(level = "trace", skip(self))]
    pub async fn exists(&self, username: &str) -> anyhow::Result<bool> {
        if self.cache.read().await.contains_key(username) {
            return Ok(true);
        }

        self.store
            .get(username)
            .await
            .map(|val| val.is_some())
            .map_err(anyhow::Error::from)
    }

    #[instrument(level = "trace", skip(self))]
    pub async fn get_user(&self, username: &str) -> Option<UserInfo> {
        self.cache.read().await.get(username).cloned()
    }

    #[instrument(level = "trace", skip(self, info))]
    pub async fn put_user(&self, username: String, info: UserInfo) -> anyhow::Result<()> {
        // Serialize first so we can bail early if there is an error
        let value = bincode::encode_to_vec(&info, bincode::config::standard())
            .context("Unable to encode data")?;
        // To be absolutely safe, fetch the current entry so we can grab its revision number so we
        // don't collide on an update
        trace!("Fetching current revision");
        let revision = self
            .store
            .entry(&username)
            .await
            .context("Unable to fetch current revision")?
            .map(|entry| entry.revision)
            .unwrap_or(0);
        trace!("Sending data to store");
        self.store
            .update(&username, value.into(), revision)
            .await
            .context("Unable to update store")?;

        trace!("Updating data in cache after successful store operation");
        {
            let mut lock = self.cache.write().await;
            if lock.insert(username, info).is_some() {
                trace!("Entry was already in cache, updating");
            } else {
                trace!("Entry was not in cache, inserting");
            }
        }

        Ok(())
    }

    #[instrument(level = "trace", skip(self))]
    pub async fn delete_user(&self, username: &str) -> anyhow::Result<()> {
        trace!("Purging user from store");
        self.store
            .purge(username)
            .await
            .context("Unable to delete user from store")?;
        trace!("Purging user from cache after successful store operation");
        if self.cache.write().await.remove(username).is_some() {
            trace!("User was in cache, removing");
        } else {
            trace!("User was not in cache");
        }
        Ok(())
    }

    #[instrument(level = "trace", skip(self))]
    pub async fn list_users(&self) -> anyhow::Result<Vec<String>> {
        Ok(self.cache.read().await.keys().cloned().collect())
    }
}

async fn initial_data_fetch(store: &Store) -> anyhow::Result<HashMap<String, UserInfo>> {
    let keys = store
        .keys()
        .await
        .context("Unable to get keys from store")?;
    let futs = keys
        .map_ok(|k| store.entry(k))
        .try_collect::<Vec<_>>()
        .await
        .context("Unable to get keys from store")?;
    futures::future::join_all(futs)
        .await
        .into_iter()
        .filter_map(|res| res.transpose())
        // Keep any that are an error (so we can handle it) or that are puts. Any that are deletes or purges we don't care about
        .filter(|res| {
            res.as_ref()
                .map(|entry| matches!(entry.operation, Operation::Put))
                .unwrap_or(true)
        })
        .map(|res| {
            res.context("Unable to get values from store")
                .and_then(|entry| {
                    let (data, _): (UserInfo, _) =
                        bincode::decode_from_slice(&entry.value, bincode::config::standard())
                            .context("Unable to decode data from store")?;
                    Ok((entry.key, data))
                })
        })
        .collect()
}

#[instrument(level = "debug", skip_all, fields(user = %entry.key, operation = ?entry.operation))]
async fn handle_entry(entry: Entry, cache: &Arc<RwLock<HashMap<String, UserInfo>>>) {
    let mut lock = cache.write().await;
    match entry.operation {
        Operation::Delete | Operation::Purge => {
            if lock.remove(&entry.key).is_none() {
                trace!(user = %entry.key, "Received purge for user that didn't exist in cache");
            }
        }
        Operation::Put => {
            trace!("Adding user information");
            let data: UserInfo =
                match bincode::decode_from_slice(&entry.value, bincode::config::standard()) {
                    Ok((data, _)) => data,
                    Err(err) => {
                        error!(%err, "Unable to decode entry received from store");
                        return;
                    }
                };
            if lock.insert(entry.key, data).is_some() {
                trace!("Updated user information");
            } else {
                trace!("Added new user")
            }
        }
    }
}
