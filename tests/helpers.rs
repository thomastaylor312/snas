use async_nats::jetstream::kv::{Config, Store};

/// Creates a new KV bucket for testing suffixed with the given name. This will first attempt to
/// delete the bucket if it exists to clean up from previous runs.
pub async fn get_store(test_name: &str) -> Store {
    let nc = async_nats::connect("127.0.0.1:4222")
        .await
        .expect("Unable to connect to NATS");
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
