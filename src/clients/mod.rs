use std::collections::BTreeSet;
use std::future::Future;

use crate::{
    admin::{PasswordResetResponse, UserResponse},
    api::VerificationResponse,
    SecureString,
};

mod nats;
#[cfg(unix)]
mod socket;

pub use nats::NatsClient;
#[cfg(unix)]
pub use socket::SocketClient;

/// A super trait for a type that implements both the [`AdminClient`] and [`UserClient`] traits.
/// This is auto-implemented for any type that implements both.
pub trait SnasClient: AdminClient + UserClient {}

impl<T> SnasClient for T where T: AdminClient + UserClient {}

/// A trait for any client that can fetch a user. This mostly exists to allow the PAM modules to be
/// able to fetch a user but not have full access to the admin API.
pub trait GetUserClient {
    /// Get the user with the given username. Returns an error if the user does not exist.
    fn get_user(&self, username: &str)
        -> impl Future<Output = anyhow::Result<UserResponse>> + Send;
}

pub trait AdminClient: GetUserClient {
    /// Create a new user with the given username and password. Returns an error if the user already
    /// exists.
    fn add_user(
        &self,
        username: &str,
        password: SecureString,
        groups: BTreeSet<String>,
        force_password_change: bool,
    ) -> impl Future<Output = anyhow::Result<()>> + Send;

    /// List all usernames.
    fn list_users(&self) -> impl Future<Output = anyhow::Result<Vec<String>>> + Send;

    /// Delete the user with the given username. Returns an error if the user does not exist.
    fn remove_user(&self, username: &str) -> impl Future<Output = anyhow::Result<()>> + Send;

    /// Reset the password of the given user. Returns an error if the user does not exist. The
    /// response will contain a randomly generated token used for logging in.
    fn reset_password(
        &self,
        username: &str,
    ) -> impl Future<Output = anyhow::Result<PasswordResetResponse>> + Send;

    /// Add the given groups to the user with the given username. Returns an error if the user does
    /// not exist. Returns the new list of groups.
    fn add_groups(
        &self,
        username: &str,
        groups: BTreeSet<String>,
    ) -> impl Future<Output = anyhow::Result<BTreeSet<String>>> + Send;

    /// Remove the given groups from the user with the given username. Returns an error if the user
    /// does not exist. Returns the new list of groups.
    fn remove_groups(
        &self,
        username: &str,
        groups: BTreeSet<String>,
    ) -> impl Future<Output = anyhow::Result<BTreeSet<String>>> + Send;
}

pub trait UserClient {
    /// Verify the given username and password, returning a [`VerificationResponse`] if successful.
    fn verify(
        &self,
        username: &str,
        password: SecureString,
    ) -> impl Future<Output = anyhow::Result<VerificationResponse>> + Send;

    /// Change the password of the given user. Returns an error if changing the password fails.
    fn change_password(
        &self,
        username: &str,
        old_password: SecureString,
        new_password: SecureString,
    ) -> impl Future<Output = anyhow::Result<()>> + Send;
}
