use serde::{Deserialize, Serialize};

use crate::types::SecureString;

/// A generic response reused for many different requests
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GenericResponse {
    pub success: bool,
    pub message: String,
}

/// A verification request for a credential challenge
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VerificationRequest {
    pub username: String,
    pub password: SecureString,
}

/// A verification response for a credential challenge
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VerificationResponse {
    pub verified: bool,
    pub message: String,
    pub needs_password_reset: bool,
    pub groups: Vec<String>,
}

/// A request to change a user's password
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PasswordChangeRequest {
    pub username: String,
    pub old_password: SecureString,
    pub new_password: SecureString,
}
