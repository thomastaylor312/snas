use std::sync::Arc;

use crate::{
    api::VerificationResponse,
    error::{HandleError, Result},
    storage::CredStore,
    SecureString,
};

#[derive(Clone)]
pub struct Handlers {
    store: Arc<CredStore>,
    default_groups: Vec<String>,
}

impl Handlers {
    /// Configures the handlers with the given store and default groups.
    pub fn new(store: CredStore, default_groups: impl Into<Vec<String>>) -> Handlers {
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

    /// Add the given user with the given password. Groups outside of the default groups must be
    /// added separately as that is an admin operation. The `needs_approval` param indicates whether
    /// the user needs to be approved before they can log in generally if it wasn't created as an
    /// admin.
    pub async fn add(
        &self,
        username: &str,
        password: SecureString,
        needs_approval: bool,
    ) -> Result<()> {
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

    /// Reset the password for the given user. Returns temporary token for use as a password
    pub async fn reset_password(&self, username: &str) -> Result<SecureString> {
        todo!()
    }

    /// Add the given groups to the user. Returns the complete list of groups after the change.
    pub async fn add_groups(
        &self,
        username: &str,
        password: SecureString,
        groups: Vec<String>,
    ) -> Result<Vec<String>> {
        todo!()
    }

    /// Remove the given groups from the user. Returns the complete list of groups after the change.
    pub async fn delete_groups(
        &self,
        username: &str,
        password: SecureString,
        groups: Vec<String>,
    ) -> Result<Vec<String>> {
        todo!()
    }

    /// Delete the given user
    pub async fn delete(&self, username: &str) -> Result<()> {
        todo!()
    }
}
