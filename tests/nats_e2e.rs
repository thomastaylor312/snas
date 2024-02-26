use snas::admin::UserAddRequest;
use snas::clients::NatsClient;

pub mod helpers;

#[tokio::test(flavor = "multi_thread")]
async fn test_user_api() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();
    let bundle = helpers::TestBundle::new("user_api", |client, handlers| async move {
        let user_api = snas::servers::nats::user::NatsUserServer::new(
            handlers,
            client,
            Some("test.user.api".to_string()),
        )
        .await
        .expect("Should be able to initialize a user server");

        user_api.run().await
    })
    .await;

    // Create a test user with the handlers before testing the user API
    let user_req = UserAddRequest {
        username: "foo".into(),
        password: "supersecure".into(),
        groups: ["foo".into()].into(),
        force_password_change: false,
    };
    bundle
        .handlers
        .add(user_req.clone())
        .await
        .expect("Should have been able to add a user");

    let user_client = NatsClient::new_with_prefix(
        bundle.client.clone(),
        Some("test.user.api".to_string()),
        Some("notused".to_string()),
    )
    .unwrap();

    // Import the trait here so it makes autocomplete nicer when selecting methods. If we import
    // admin and user traits globally, all the methods show up
    use snas::clients::UserClient;

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

// TODO: password reset flow
// TODO: admin server
