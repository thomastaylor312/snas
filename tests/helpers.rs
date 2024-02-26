use std::future::Future;

use async_nats::jetstream::kv::{Config, Store};
use snas::storage::CredStore;
use tokio::task::JoinHandle;

/// Creates a new KV bucket for testing suffixed with the given name. This will first attempt to
/// delete the bucket if it exists to clean up from previous runs.
pub async fn get_store(test_name: &str) -> Store {
    let nc = get_client().await;
    let js = async_nats::jetstream::new(nc);

    let bucket_name = format!("testing_{test_name}");
    // Always try to delete the bucket to clean up from previous runs. It is easier than trying to
    // handle an async drop
    let _ = js.delete_key_value(&bucket_name).await;

    js.create_key_value(Config {
        bucket: bucket_name,
        description: format!("A snes test bucket for {test_name}"),
        storage: async_nats::jetstream::stream::StorageType::Memory,
        ..Default::default()
    })
    .await
    .expect("Unable to create bucket")
}

pub async fn get_store_from_client(client: async_nats::Client, test_name: &str) -> Store {
    let js = async_nats::jetstream::new(client);
    let bucket_name = format!("testing_{test_name}");
    // Always try to delete the bucket to clean up from previous runs. It is easier than trying to
    // handle an async drop
    let _ = js.delete_key_value(&bucket_name).await;

    js.create_key_value(Config {
        bucket: bucket_name,
        description: format!("A snes test bucket for {test_name}"),
        storage: async_nats::jetstream::stream::StorageType::Memory,
        ..Default::default()
    })
    .await
    .expect("Unable to create bucket")
}

pub async fn get_client() -> async_nats::Client {
    async_nats::connect("127.0.0.1:4222")
        .await
        .expect("Unable to connect to NATS")
}

pub struct TestBundle {
    pub client: async_nats::Client,
    pub handle: JoinHandle<anyhow::Result<()>>,
    pub handlers: snas::handlers::Handlers,
}

impl Drop for TestBundle {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

impl TestBundle {
    pub async fn new<F, Fut>(test_name: &str, constructor: F) -> Self
    where
        F: FnOnce(async_nats::Client, snas::handlers::Handlers) -> Fut,
        Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        let client = get_client().await;
        let store = CredStore::new(get_store_from_client(client.clone(), test_name).await)
            .await
            .expect("Should have been able to initialize a CredStore");
        let handlers = snas::handlers::Handlers::new(store);
        let fut = constructor(client.clone(), handlers.clone());
        let handle = tokio::spawn(fut);
        Self {
            client,
            handle,
            handlers,
        }
    }
}
