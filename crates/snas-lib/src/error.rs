pub type Result<T> = std::result::Result<T, HandleError>;

#[derive(Debug, thiserror::Error)]
pub enum HandleError {
    /// The username requested for creation already exists
    #[error("Username already exists")]
    UsernameTaken,
    /// An invalid password was given
    #[error("Invalid username or password")]
    InvalidCredentials,
    /// The password was reset and has expired
    #[error("Password reset has expired")]
    PasswordResetExpired,
    /// The username sent for the requested operation does not exist
    #[error("Username does not exist")]
    UsernameDoesNotExist,
    /// Errors that occur when interacting with storage or other parts of the system
    #[error(transparent)]
    SystemError(#[from] anyhow::Error),
}
