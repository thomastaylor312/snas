use std::{collections::BTreeSet, fmt::Debug, time::Duration};

use bincode::{Decode, Encode};

pub mod admin;
pub mod api;
mod secure;

pub use secure::*;
use serde::{Deserialize, Serialize};

/// Information necessary to verify a user's credentials and identify their groups
#[derive(Debug, Clone, Encode, Decode)]
pub struct UserInfo {
    // NOTE(thomastaylor312): Because we're using Argon2, the salt is included in the hashed
    // password
    pub hashed_password: SecureString,
    pub password_reset: Option<PasswordResetPhase>,
    pub groups: BTreeSet<String>,
}

/// The current state of a user's password reset process
#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
pub enum PasswordResetPhase {
    /// The user's password has been reset, but they still need to log in to change it. Will expire
    /// at the given duration (as measured in seconds since the unix epoch)
    Reset(Duration),
    /// The user has logged in once, and should be prompted to change their password. Will expire at
    /// the given duration (as measured in seconds since the unix epoch)
    InitialLogin(Duration),
    /// The password reset has expired, or the user has logged in a second time without changing
    /// their password and will need to be reset again
    Locked,
}
