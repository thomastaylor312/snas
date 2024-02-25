use snas::{admin::UserAddRequest, storage::CredStore};

pub mod helpers;

#[tokio::test]
async fn test_user_api() {
    let client = helpers::get_client().await;
    let store = CredStore::new(helpers::get_store_from_client(client.clone(), "user_api").await)
        .await
        .expect("Should have been able to initialize a CredStore");
    let handlers = snas::handlers::Handlers::new(store);

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

    let user_api = snas::servers::nats::user::NatsUserServer::new(
        handlers,
        client,
        Some("test.user.api".to_string()),
    )
    .await
    .expect("Should be able to initialize a user server");

    // TODO: Finish tests here for user API
}

// TODO: password reset flow
// TODO: admin server
