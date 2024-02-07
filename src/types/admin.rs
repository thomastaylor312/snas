use std::{collections::BTreeSet, time::Duration};

use serde::{Deserialize, Serialize};

use crate::{types::SecureString, PasswordResetPhase};

/// A request to create a new user with the given password and groups. This is for admin use only as
/// users should not be able to create new groups
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AdminUserAddRequest {
    pub username: String,
    pub password: SecureString,
    pub groups: BTreeSet<String>,
    pub force_password_change: bool,
}

/// A request to get a specific user
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UserGetRequest {
    pub username: String,
}

/// A request to delete a user
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UserDeleteRequest {
    pub username: String,
}

/// A user object returned in get requests
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UserResponse {
    pub username: String,
    pub groups: BTreeSet<String>,
    pub password_change_phase: Option<PasswordResetPhase>,
}

/// A request to add groups to a user
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GroupModifyRequest {
    pub username: String,
    pub groups: BTreeSet<String>,
}

/// A request to reset a user's password
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PasswordResetRequest {
    pub username: String,
}

/// Response for a password reset. Will contain a randomly generated token used for logging in
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PasswordResetResponse {
    pub temp_password: SecureString,
    pub expires_at: Duration,
}
