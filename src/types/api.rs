use std::collections::BTreeSet;

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

    /// Helper function to convert the response into an `anyhow::Result`
    pub fn into_result_empty(self) -> anyhow::Result<()> {
        self.into_result().map(|_| ())
    }
}

impl<T: 'static> GenericResponse<T> {
    /// Helper function to convert the response into an `anyhow::Result` with the contained response
    pub fn into_result(self) -> anyhow::Result<Option<T>> {
        if self.success {
            Ok(self.response)
        } else {
            Err(anyhow::anyhow!(self.message))
        }
    }

    /// Helper function similar to [`into_result`](Self::into_response) but returns an error if the inner response is None
    pub fn into_result_required(self) -> anyhow::Result<T> {
        match (self.success, self.response) {
            (true, Some(response)) => Ok(response),
            (true, None) => Err(anyhow::anyhow!(
                "Request was successful but contained no response"
            )),
            (false, None) | (false, Some(_)) => Err(anyhow::anyhow!(self.message)),
        }
    }
}

impl<T: 'static> From<GenericResponse<T>> for anyhow::Result<Option<T>> {
    fn from(response: GenericResponse<T>) -> Self {
        response.into_result()
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
    /// Whether the credentials were valid
    pub valid: bool,
    pub message: String,
    pub needs_password_reset: bool,
    pub groups: BTreeSet<String>,
}

/// A request to change a user's password
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PasswordChangeRequest {
    pub username: String,
    pub old_password: SecureString,
    pub new_password: SecureString,
}
