use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{types::SecureString, PasswordResetPhase};

/// A request to create a new user with the given password and groups. This is for admin use only as
/// users should not be able to create new groups
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AdminUserAddRequest {
    pub username: String,
    pub password: SecureString,
    pub groups: Vec<String>,
    pub force_password_change: bool,
}

/// A request to approve or unapprove a user.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UserApproveRequest {
    pub username: String,
    pub approve: bool,
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
    pub groups: Vec<String>,
    pub approved: bool,
    pub password_change_phase: Option<PasswordResetPhase>,
}

/// A request to add groups to a user
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GroupAddRequest {
    pub username: String,
    pub groups: Vec<String>,
}

/// A request to delete groups from a user
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GroupDeleteRequest {
    pub username: String,
    pub groups: Vec<String>,
}

/// Response returned from group modification
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GroupResponse {
    pub groups: Vec<String>,
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
