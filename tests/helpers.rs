use std::future::Future;

use async_nats::jetstream::kv::{Config, Store};
use snas::storage::CredStore;
use snas::types::admin::UserAddRequest;
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
    pub store: async_nats::jetstream::kv::Store,
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
        let nats_store = get_store_from_client(client.clone(), test_name).await;
        let store = CredStore::new(nats_store.clone())
            .await
            .expect("Should have been able to initialize a CredStore");
        let handlers = snas::handlers::Handlers::new(store);
        let fut = constructor(client.clone(), handlers.clone());
        let handle = tokio::spawn(fut);
        // Give things a sec to put their pants on
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        Self {
            client,
            store: nats_store,
            handle,
            handlers,
        }
    }
}

pub struct TestSocketBundle {
    pub handle: JoinHandle<anyhow::Result<()>>,
    pub handlers: snas::handlers::Handlers,
    pub socket_path: std::path::PathBuf,
    _temp_dir: tempfile::TempDir,
}

impl Drop for TestSocketBundle {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

impl TestSocketBundle {
    pub async fn new(test_name: &str) -> Self {
        let client = get_client().await;
        let nats_store = get_store_from_client(client, test_name).await;
        let store = CredStore::new(nats_store.clone())
            .await
            .expect("Should have been able to initialize a CredStore");
        let handlers = snas::handlers::Handlers::new(store);
        let temp_dir = tempfile::tempdir().expect("Unable to create temp dir");
        let socket_path = temp_dir.path().join(test_name);
        let server = snas::servers::socket::SocketUserServer::new(handlers.clone(), &socket_path)
            .await
            .expect("Unable to create socket");
        let handle = tokio::spawn(server.run());
        // Give things a sec to put their pants on
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        Self {
            handle,
            handlers,
            socket_path,
            _temp_dir: temp_dir,
        }
    }
}

pub async fn assert_user_server<T: snas::clients::UserClient>(
    user_client: T,
    handlers: &snas::handlers::Handlers,
) {
    // Create a test user with the handlers before testing the user API
    let user_req = UserAddRequest {
        username: "foo".into(),
        password: "supersecure".into(),
        groups: ["foo".into()].into(),
        force_password_change: false,
    };
    handlers
        .add(user_req.clone())
        .await
        .expect("Should have been able to add a user");

    let resp = user_client
        .verify(&user_req.username, user_req.password.clone())
        .await
        .expect("Should be able to verify a user");
    assert!(resp.valid, "Should verify with correct password");
    assert!(
        !resp.needs_password_reset,
        "User should not need a password reset"
    );
    assert_eq!(
        resp.groups, user_req.groups,
        "User should have the correct groups"
    );

    // Test a valid user with invalid password
    let resp = user_client
        .verify(&user_req.username, "invalid".into())
        .await
        .expect("Should be able to perform verify request");
    assert!(!resp.valid, "Invalid password should not verify user");

    // Now test with an invalid username
    let resp = user_client
        .verify("invalid", user_req.password.clone())
        .await
        .expect("Should be able to perform verify request with invalid username");
    assert!(!resp.valid, "Invalid username should not verify user");

    // Try changing the password with an invalid password
    user_client
        .change_password(&user_req.username, "invalid".into(), "newpassword".into())
        .await
        .expect_err("Invalid password should error on change password");

    // Try changing the password successfully and then verify with new password
    user_client
        .change_password(
            &user_req.username,
            user_req.password.clone(),
            "newpassword".into(),
        )
        .await
        .expect("Should be able to change password");

    let resp = user_client
        .verify(&user_req.username, "newpassword".into())
        .await
        .expect("Should be able to verify with new password");

    assert!(resp.valid, "Should verify with new password");
}
