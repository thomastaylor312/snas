use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::types::SecureString;

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
