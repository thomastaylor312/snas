use snas_lib::clients::SocketClient;

pub mod helpers;

#[tokio::test(flavor = "multi_thread")]
async fn test_user_api() {
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .try_init();
    let bundle = helpers::TestSocketBundle::new("socket_user_api").await;

    let user_client = SocketClient::new(&bundle.socket_path)
        .await
        .expect("Should be able to create a client");

    helpers::assert_user_server(user_client, &bundle.handlers).await;
}
