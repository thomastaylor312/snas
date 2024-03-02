use std::time::{SystemTime, UNIX_EPOCH};

use futures::future::Either;
use futures::FutureExt;
use snas::admin::UserAddRequest;
use snas::clients::NatsClient;
use snas::{PasswordResetPhase, UserInfo};

pub mod helpers;

#[tokio::test(flavor = "multi_thread")]
async fn test_user_api() {
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .try_init();
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

#[tokio::test(flavor = "multi_thread")]
async fn test_admin_api() {
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .try_init();
    let bundle = helpers::TestBundle::new("admin_api", |client, handlers| async move {
        let admin_api = snas::servers::nats::admin::NatsAdminServer::new(
            handlers,
            client,
            Some("test.admin.api".to_string()),
        )
        .await
        .expect("Should be able to initialize a admin server");

        admin_api.run().await
    })
    .await;

    let admin_client = NatsClient::new_with_prefix(
        bundle.client.clone(),
        Some("notused".to_string()),
        Some("test.admin.api".to_string()),
    )
    .unwrap();

    use snas::clients::AdminClient;

    admin_client
        .add_user("foo", "easy123".into(), ["foo".into()].into(), false)
        .await
        .expect("Should be able to add user");

    let user = admin_client
        .get_user("foo")
        .await
        .expect("Should be able to get user");
    assert_eq!(
        user.username, "foo",
        "User should have the correct username"
    );
    assert_eq!(
        user.groups,
        ["foo".into()].into(),
        "User should have the correct groups"
    );
    assert!(
        user.password_change_phase.is_none(),
        "User should not be locked"
    );

    // Getting a non-existent user should error
    admin_client
        .get_user("bar")
        .await
        .expect_err("Should not be able to get non-existent user");

    // Add one more user for testing
    admin_client
        .add_user("bar", "easy123".into(), ["bar".into()].into(), false)
        .await
        .expect("Should be able to add user");

    // Test listing users
    let mut list_users_result = admin_client
        .list_users()
        .await
        .expect("Should be able to list users");
    assert_eq!(list_users_result.len(), 2);
    list_users_result.sort();
    assert_eq!(
        list_users_result,
        ["bar".to_string(), "foo".to_string()],
        "Should have the correct users"
    );

    // Test removing a user
    admin_client
        .remove_user("foo")
        .await
        .expect("Should be able to remove user");

    admin_client
        .get_user("foo")
        .await
        .expect_err("Should not be able to get deleted user");

    // Test resetting a user's password
    admin_client
        .reset_password("bar")
        .await
        .expect("Should be able to reset password");

    let user = admin_client
        .get_user("bar")
        .await
        .expect("Should be able to get user");
    assert!(
        matches!(
            user.password_change_phase
                .expect("Should have a password change phase"),
            PasswordResetPhase::Reset(_)
        ),
        "User should be in the reset phase",
    );

    // Test adding groups to a user
    let add_groups_result = admin_client
        .add_groups("bar", ["group1".to_string(), "group2".to_string()].into())
        .await
        .expect("Should be able to add groups");
    assert_eq!(
        add_groups_result,
        ["group1".into(), "group2".into(), "bar".into()].into(),
        "Should have the correct groups after add"
    );

    // Make sure when we fetch the user, the groups are correct
    let user = admin_client
        .get_user("bar")
        .await
        .expect("Should be able to get user");

    assert_eq!(
        user.groups,
        ["bar".into(), "group1".into(), "group2".into()].into(),
        "Should have the correct groups after add"
    );

    // Test removing groups from a user
    let remove_groups_result = admin_client
        .remove_groups("bar", ["group1".into()].into())
        .await
        .expect("Should be able to remove groups");
    assert_eq!(
        remove_groups_result,
        ["group2".into(), "bar".into()].into(),
        "Should have the correct groups after delete"
    );

    // Make sure when we fetch the user, the groups are correct
    let user = admin_client
        .get_user("bar")
        .await
        .expect("Should be able to get user");

    assert_eq!(
        user.groups,
        ["bar".into(), "group2".into()].into(),
        "Should have the correct groups after delete"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_password_reset_flow() {
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .try_init();
    let bundle = helpers::TestBundle::new("password_reset_flow", |client, handlers| async move {
        let admin_api = snas::servers::nats::admin::NatsAdminServer::new(
            handlers.clone(),
            client.clone(),
            Some("test.admin.password".to_string()),
        )
        .await
        .expect("Should be able to initialize a admin server");
        let user_api = snas::servers::nats::user::NatsUserServer::new(
            handlers,
            client,
            Some("test.user.password".to_string()),
        )
        .await
        .expect("Should be able to initialize a user server");

        futures::future::select_ok([
            Either::Left(admin_api.run().boxed()),
            Either::Right(user_api.run().boxed()),
        ])
        .map(|val| val.map(|data| data.0))
        .await
    })
    .await;

    let client = NatsClient::new_with_prefix(
        bundle.client.clone(),
        Some("test.user.password".to_string()),
        Some("test.admin.password".to_string()),
    )
    .unwrap();

    use snas::clients::{AdminClient, UserClient};

    // Add a user for the test
    client
        .add_user("foo", "easy123".into(), ["foo".into()].into(), true)
        .await
        .expect("Should be able to add user");

    // Try doing a password reset on the first login
    client
        .change_password("foo", "easy123".into(), "easy1234".into())
        .await
        .expect("Should be able to change password");

    let resp = client
        .verify("foo", "easy1234".into())
        .await
        .expect("Should be able to log in after reset");
    assert!(
        !resp.needs_password_reset,
        "Should not need a password reset"
    );
    assert!(resp.valid, "Should be able to log in after reset");

    // Force a password reset, log in once, and then try to change the password
    let password = client
        .reset_password("foo")
        .await
        .expect("Should be able to reset password")
        .temp_password;
    let resp = client
        .verify("foo", password.clone())
        .await
        .expect("Should be able to log in");
    assert!(resp.needs_password_reset, "Should need a password reset");
    assert!(resp.valid, "Should be able to log in");

    let user = client
        .get_user("foo")
        .await
        .expect("Should be able to get user");
    assert!(
        matches!(
            user.password_change_phase.unwrap(),
            PasswordResetPhase::InitialLogin(_)
        ),
        "User should be in the initial login phase",
    );

    // Now try to change the password and make sure we still can
    client
        .change_password("foo", password, "easy12345".into())
        .await
        .expect("Should be able to change password");

    // Try to log in with the new password
    let resp = client
        .verify("foo", "easy12345".into())
        .await
        .expect("Should be able to log in after reset");
    assert!(
        !resp.needs_password_reset,
        "Should not need a password reset"
    );
    assert!(resp.valid, "Should be able to log in after reset");

    // Reset one more time and then try to log in twice
    let password = client
        .reset_password("foo")
        .await
        .expect("Should be able to reset password")
        .temp_password;
    let resp = client
        .verify("foo", password.clone())
        .await
        .expect("Should be able to log in");
    assert!(resp.needs_password_reset, "Should need a password reset");
    assert!(resp.valid, "Should be able to log in");

    let resp = client
        .verify("foo", password.clone())
        .await
        .expect("Should be able to verify");
    assert!(
        !resp.valid,
        "Should not be able to log in after second login attempt"
    );

    let user = client
        .get_user("foo")
        .await
        .expect("Should be able to get user");
    assert!(
        matches!(
            user.password_change_phase.unwrap(),
            PasswordResetPhase::Locked,
        ),
        "User should be in the locked phase",
    );

    // This is a little janky, but we need to set an already expired timestamp for testing expiry
    let raw = bundle
        .store
        .get("foo")
        .await
        .expect("Should be able to fetch data from store")
        .expect("User should exist in store");
    let (mut data, _): (UserInfo, _) =
        bincode::decode_from_slice(&raw, bincode::config::standard()).unwrap();
    data.password_reset = Some(PasswordResetPhase::Reset(
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap() - std::time::Duration::from_secs(300),
    ));
    let encoded = bincode::encode_to_vec(&data, bincode::config::standard()).unwrap();
    bundle
        .store
        .put("foo", encoded.into())
        .await
        .expect("Should be able to put data in store");

    // Try to log in
    let resp = client
        .verify("foo", password)
        .await
        .expect("Should be able to verify");
    assert!(
        !resp.valid,
        "Should not be able to log in after password reset has expired"
    );
    assert!(
        resp.needs_password_reset,
        "Should need a password reset after password reset has expired"
    );
}
