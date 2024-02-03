use serde::{Deserialize, Serialize};

use crate::types::SecureString;

/// A generic response reused for many different requests
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GenericResponse<T: 'static> {
    /// Whether the request succeeded
    pub success: bool,
    /// A message with additional context about the response
    pub message: String,
    /// The response data, if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<T>,
}

impl GenericResponse<EmptyResponse> {
    /// Create a new response with no response data
    pub fn new(success: bool, message: String) -> Self {
        Self {
            success,
            message,
            response: None,
        }
    }
}

/// An empty type used when returning a response with no data
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EmptyResponse;

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
