use std::{collections::BTreeSet, sync::Arc};

use crate::{
    admin::{AdminUserAddRequest, PasswordResetResponse, UserResponse},
    api::VerificationResponse,
    error::Result,
    storage::CredStore,
    SecureString,
};

#[derive(Clone)]
pub struct Handlers {
    store: Arc<CredStore>,
    default_groups: BTreeSet<String>,
}

impl Handlers {
    /// Configures the handlers with the given store and default groups.
    pub fn new(store: CredStore, default_groups: impl Into<BTreeSet<String>>) -> Handlers {
        Handlers {
            store: Arc::new(store),
            default_groups: default_groups.into(),
        }
    }

    /// Verify the given username and password. Returns the groups the user is a member of and
    /// whether or not the user was verified.
    pub async fn verify(
        &self,
        username: &str,
        password: SecureString,
    ) -> Result<VerificationResponse> {
        // TODO: If the user has a "needs reset" set, the user must reset their password before continuing on
        todo!()
    }

    /// Add the given user to the system. This is meant to be used by admins only
    pub async fn add(&self, req: AdminUserAddRequest) -> Result<()> {
        todo!()
    }

    /// Change the password for the given user. Requires the current password.
    pub async fn change_password(
        &self,
        username: &str,
        current_password: SecureString,
        new_password: SecureString,
    ) -> Result<()> {
        todo!()
    }

    /// Set the approval flag on a user
    pub async fn set_approval(&self, username: &str, approved: bool) -> Result<()> {
        todo!()
    }

    /// Reset the password for the given user. Returns temporary token for use as a password
    pub async fn reset_password(&self, username: &str) -> Result<PasswordResetResponse> {
        todo!()
    }

    /// Add the given groups to the user. Returns the complete list of groups after the change.
    pub async fn add_groups(
        &self,
        username: &str,
        groups: BTreeSet<String>,
    ) -> Result<BTreeSet<String>> {
        todo!()
    }

    /// Remove the given groups from the user. Returns the complete list of groups after the change.
    pub async fn delete_groups(
        &self,
        username: &str,
        groups: BTreeSet<String>,
    ) -> Result<BTreeSet<String>> {
        todo!()
    }

    /// Delete the given user, returning None if the user didn't exist
    pub async fn delete(&self, username: &str) -> Result<()> {
        todo!()
    }

    /// Get information for the given user. Returns None if the user doesn't exist.
    pub async fn get(&self, username: &str) -> Result<UserResponse> {
        todo!()
    }

    /// Get all usernames
    pub async fn list(&self) -> Result<Vec<String>> {
        todo!()
    }
}
