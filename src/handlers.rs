use std::{
    collections::BTreeSet,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand::rngs::OsRng;
use rand::{distributions::Alphanumeric, Rng};
use tracing::error;

use crate::{
    admin::{AdminUserAddRequest, PasswordResetResponse, UserResponse},
    api::VerificationResponse,
    error::{HandleError, Result},
    storage::CredStore,
    PasswordResetPhase, SecureString, UserInfo,
};

// TODO(thomastaylor312): We eventually should make this configurable
const DEFAULT_RESET_EXPIRY: Duration = Duration::from_secs(60 * 60 * 24);

#[derive(Clone)]
pub struct Handlers {
    store: Arc<CredStore>,
}

impl Handlers {
    /// Configures the handlers with the given store and default groups.
    pub fn new(store: CredStore) -> Handlers {
        Handlers {
            store: Arc::new(store),
        }
    }

    /// Verify the given username and password. Returns the groups the user is a member of and
    /// whether or not the user was verified.
    pub async fn verify(
        &self,
        username: &str,
        password: SecureString,
    ) -> Result<VerificationResponse> {
        let current_user = self
            .store
            .get_user(username)
            .await
            .ok_or_else(|| HandleError::UsernameDoesNotExist)?;

        let current_user = self
            .enforce_login_state(username, current_user, true)
            .await?;

        verify_password(&current_user, &password)?;
        Ok(VerificationResponse {
            message: "Successfully verified".to_string(),
            needs_password_reset: current_user.password_reset.is_some(),
            groups: current_user.groups,
        })
    }

    /// Add the given user to the system. This is meant to be used by admins only
    pub async fn add(&self, req: AdminUserAddRequest) -> Result<()> {
        if self.store.exists(&req.username).await? {
            return Err(HandleError::UsernameTaken);
        }

        let hashed_password = hash_password(&req.password)?;
        let password_reset = if req.force_password_change {
            Some(PasswordResetPhase::Reset(get_expiry_duration(
                DEFAULT_RESET_EXPIRY,
            )?))
        } else {
            None
        };
        let user_data = UserInfo {
            hashed_password,
            password_reset,
            groups: req.groups,
        };

        self.store
            .put_user(req.username, user_data)
            .await
            .map_err(HandleError::from)
    }

    /// Change the password for the given user. Requires the current password.
    pub async fn change_password(
        &self,
        username: &str,
        current_password: SecureString,
        new_password: SecureString,
    ) -> Result<()> {
        let current_user = self
            .store
            .get_user(username)
            .await
            .ok_or_else(|| HandleError::UsernameDoesNotExist)?;

        let mut current_user = self
            .enforce_login_state(username, current_user, true)
            .await?;

        verify_password(&current_user, &current_password)?;

        let hashed_password = hash_password(&new_password)?;
        current_user.hashed_password = hashed_password;
        // State should now be reset to None if we got to this point
        current_user.password_reset = None;

        self.store
            .put_user(username.to_owned(), current_user)
            .await
            .map_err(HandleError::from)
    }

    /// Reset the password for the given user. Returns temporary token for use as a password
    pub async fn reset_password(&self, username: &str) -> Result<PasswordResetResponse> {
        let mut current_user = self
            .store
            .get_user(username)
            .await
            .ok_or_else(|| HandleError::UsernameDoesNotExist)?;
        // Generate a random string using OsRng to use as a password
        let new_password: SecureString = std::iter::repeat(())
            .map(|()| OsRng.sample(Alphanumeric))
            .map(char::from)
            .take(32)
            .collect::<String>()
            .into();
        let hashed_password = hash_password(&new_password)?;
        let expiry = get_expiry_duration(DEFAULT_RESET_EXPIRY)?;

        // Store the new password and expiry in the store
        current_user.hashed_password = hashed_password;
        current_user.password_reset = Some(PasswordResetPhase::Reset(expiry));
        self.store
            .put_user(username.to_owned(), current_user)
            .await?;

        Ok(PasswordResetResponse {
            temp_password: new_password,
            expires_at: expiry,
        })
    }

    /// Add the given groups to the user. Returns the complete list of groups after the change.
    pub async fn add_groups(
        &self,
        username: &str,
        groups: BTreeSet<String>,
    ) -> Result<BTreeSet<String>> {
        let mut current_user = self
            .store
            .get_user(username)
            .await
            .ok_or_else(|| HandleError::UsernameDoesNotExist)?;
        current_user.groups.extend(groups);
        let resp = current_user.groups.clone();

        self.store
            .put_user(username.to_owned(), current_user)
            .await
            .map(|_| resp)
            .map_err(HandleError::from)
    }

    /// Remove the given groups from the user. Returns the complete list of groups after the change.
    pub async fn delete_groups(
        &self,
        username: &str,
        groups: BTreeSet<String>,
    ) -> Result<BTreeSet<String>> {
        let mut current_user = self
            .store
            .get_user(username)
            .await
            .ok_or_else(|| HandleError::UsernameDoesNotExist)?;
        current_user.groups = current_user.groups.difference(&groups).cloned().collect();

        let resp = current_user.groups.clone();
        self.store
            .put_user(username.to_owned(), current_user)
            .await
            .map(|_| resp)
            .map_err(HandleError::from)
    }

    /// Delete the given user
    pub async fn delete(&self, username: &str) -> Result<()> {
        self.store
            .delete_user(username)
            .await
            .map_err(HandleError::from)
    }

    /// Get information for the given user. Returns None if the user doesn't exist.
    pub async fn get(&self, username: &str) -> Result<UserResponse> {
        match self.store.get_user(username).await {
            Some(user) => Ok(UserResponse {
                username: username.to_owned(),
                groups: user.groups,
                password_change_phase: user.password_reset,
            }),
            None => Err(HandleError::UsernameDoesNotExist),
        }
    }

    /// Get all usernames
    pub async fn list(&self) -> Result<Vec<String>> {
        self.store.list_users().await.map_err(HandleError::from)
    }

    /// Checks if a password reset is needed for the given user and updates the current phase as
    /// needed. Returns the updated user object if successful
    async fn enforce_login_state(
        &self,
        username: &str,
        mut user: UserInfo,
        is_password_change: bool,
    ) -> Result<UserInfo> {
        let (user_data, allowed) = match user.password_reset {
            Some(PasswordResetPhase::Reset(expiry)) => {
                let now = current_time()?;
                if now < expiry {
                    user.password_reset = Some(PasswordResetPhase::InitialLogin(expiry));
                    (user, true)
                } else {
                    user.password_reset = Some(PasswordResetPhase::Locked);
                    (user, false)
                }
            }
            Some(PasswordResetPhase::InitialLogin(expiry)) if is_password_change => {
                let now = current_time()?;
                if now >= expiry {
                    user.password_reset = Some(PasswordResetPhase::Locked);
                    (user, false)
                } else {
                    // If things haven't expired, then we can just return the user. If for some reason
                    // they mistyped their temp password, then they should be able to try again anyway
                    return Ok(user);
                }
            }
            Some(PasswordResetPhase::InitialLogin(_)) => {
                // If this is not a password reset, then we should update to locked and deny, no
                // matter what the expiry is
                user.password_reset = Some(PasswordResetPhase::Locked);
                (user, false)
            }
            Some(PasswordResetPhase::Locked) => {
                // If we're locked, no update is needed, just deny
                return Err(HandleError::PasswordResetExpired);
            }
            None => {
                // In this case we don't need to modify anything and can just return the user
                return Ok(user);
            }
        };
        self.store
            .put_user(username.to_owned(), user_data.clone())
            .await?;

        if !allowed {
            return Err(HandleError::PasswordResetExpired);
        }
        Ok(user_data)
    }
}

fn get_expiry_duration(time_to_expire: Duration) -> anyhow::Result<Duration> {
    current_time().map(|t| t + time_to_expire)
}

fn hash_password(password: &SecureString) -> Result<SecureString> {
    let salt = SaltString::generate(&mut OsRng);
    let argon = Argon2::default();
    argon
        .hash_password(password.as_ref(), &salt)
        .map_err(|err| {
            error!(%err, "Error occurred when hashing password");
            HandleError::SystemError(anyhow::anyhow!("Error when hashing"))
        })
        .map(|hashed| hashed.to_string().into())
}

// Verifies that the given password matches the stored password for the given user. Returns an error
// if validation fails or another error occurs
fn verify_password(user: &UserInfo, password: &SecureString) -> Result<()> {
    let password_hash = match PasswordHash::new(user.hashed_password.as_ref()) {
        Ok(hash) => hash,
        Err(err) => {
            error!(%err, "Error occurred when parsing password hash. This is likely a data corruption issue!");
            return Err(HandleError::SystemError(anyhow::anyhow!(
                "Error when reading user"
            )));
        }
    };
    Argon2::default()
        .verify_password(password.as_ref(), &password_hash)
        .map_err(|_| HandleError::InvalidCredentials)
}

fn current_time() -> anyhow::Result<Duration> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("Unable to calculate current system time")
}
