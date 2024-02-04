use snas::{storage::CredStore, UserInfo};

mod helpers;

#[tokio::test]
async fn test_crud() {
    let store = CredStore::new(helpers::get_store("storage_crud").await)
        .await
        .expect("Should have been able to initialize a CredStore");

    assert!(
        store.get_user("foo").await.is_none(),
        "Users should not exist"
    );

    let mut foo_user = UserInfo {
        hashed_password: "bar".into(),
        password_reset: None,
        needs_approval: false,
        groups: ["foo".into()].into(),
    };

    store
        .put_user("foo".into(), foo_user.clone())
        .await
        .expect("Should have been able to insert a user");

    let user = store
        .get_user("foo")
        .await
        .expect("Should have been able to get a user");
    assert_eq!(
        user.hashed_password, foo_user.hashed_password,
        "Users should be equal"
    );
    assert_eq!(user.groups, foo_user.groups, "Users should be equal");

    foo_user.groups.insert("bar".into());

    store
        .put_user("foo".into(), foo_user.clone())
        .await
        .expect("Should be able to update user");

    let user = store
        .get_user("foo")
        .await
        .expect("Should have been able to get a user");
    assert_eq!(
        user.groups, foo_user.groups,
        "Users should be equal after update"
    );

    store
        .delete_user("foo")
        .await
        .expect("Should have been able to delete a user");

    assert!(
        store.get_user("foo").await.is_none(),
        "Users should not exist"
    );
}

#[tokio::test]
async fn test_initialization() {
    let bucket = helpers::get_store("storage_crud").await;
    let store = CredStore::new(bucket.clone())
        .await
        .expect("Should have been able to initialize a CredStore");

    let foo_user = UserInfo {
        hashed_password: "bar".into(),
        password_reset: None,
        needs_approval: false,
        groups: ["foo".into()].into(),
    };
    let bar_user = UserInfo {
        hashed_password: "baz".into(),
        password_reset: None,
        needs_approval: false,
        groups: ["foo".into()].into(),
    };
    // Insert some data
    store
        .put_user("foo".into(), foo_user.clone())
        .await
        .expect("Should be able to insert a user");
    store
        .put_user("bar".into(), bar_user.clone())
        .await
        .expect("Should be able to insert a user");

    // Then drop the store and re-initialize it
    drop(store);

    let store = CredStore::new(bucket)
        .await
        .expect("Should have been able to initialize a CredStore");

    let user = store
        .get_user("foo")
        .await
        .expect("Should have been able to get a user after initialization");
    assert_eq!(
        user.hashed_password, foo_user.hashed_password,
        "Users should be equal"
    );
    assert_eq!(user.groups, foo_user.groups, "Users should be equal");

    let user = store
        .get_user("bar")
        .await
        .expect("Should have been able to get a user after initialization");
    assert_eq!(
        user.hashed_password, bar_user.hashed_password,
        "Users should be equal"
    );
    assert_eq!(user.groups, bar_user.groups, "Users should be equal");
}

#[tokio::test]
async fn test_sync() {
    let bucket = helpers::get_store("storage_sync").await;
    let main_store = CredStore::new(bucket.clone())
        .await
        .expect("Should have been able to initialize a CredStore");

    let reflected_store = CredStore::new(bucket)
        .await
        .expect("Should have been able to initialize a CredStore");

    // Insert some data in the main store, sleep, then see that it was reflected in the reflected store
    let mut foo_user = UserInfo {
        hashed_password: "bar".into(),
        password_reset: None,
        needs_approval: false,
        groups: ["foo".into()].into(),
    };
    main_store
        .put_user("foo".into(), foo_user.clone())
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    let user = reflected_store
        .get_user("foo")
        .await
        .expect("User should exist in reflected store");
    assert_eq!(
        user.hashed_password, foo_user.hashed_password,
        "Users should be equal"
    );
    assert_eq!(user.groups, foo_user.groups, "Users should be equal");

    // Update some data in the main store, sleep, then see that it was reflected in the reflected store
    foo_user.groups.insert("bar".into());
    main_store
        .put_user("foo".into(), foo_user.clone())
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    let user = reflected_store
        .get_user("foo")
        .await
        .expect("User should exist in reflected store");
    assert_eq!(
        user.groups, foo_user.groups,
        "Users should be equal after update"
    );

    // Delete some data in the main store, sleep, then see that it was reflected in the reflected store

    main_store.delete_user("foo").await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    assert!(
        reflected_store.get_user("foo").await.is_none(),
        "User should not exist in reflected store"
    );
}
