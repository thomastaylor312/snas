use std::collections::BTreeSet;

use anyhow::Context;
use async_nats::Client;
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    admin::{
        GroupModifyRequest, PasswordResetRequest, PasswordResetResponse, UserAddRequest,
        UserDeleteRequest, UserGetRequest, UserResponse,
    },
    api::{GenericResponse, PasswordChangeRequest, VerificationRequest, VerificationResponse},
    SecureString, DEFAULT_ADMIN_NATS_SUBJECT_PREFIX, DEFAULT_USER_NATS_SUBJECT_PREFIX,
};

pub struct NatsClient {
    client: Client,
    user_topic_prefix: String,
    admin_topic_prefix: String,
}

impl NatsClient {
    /// Creates a new client, using the default topic prefixes.
    pub fn new(client: Client) -> Self {
        Self {
            client,
            user_topic_prefix: DEFAULT_USER_NATS_SUBJECT_PREFIX.to_string(),
            admin_topic_prefix: DEFAULT_ADMIN_NATS_SUBJECT_PREFIX.to_string(),
        }
    }

    /// Creates a new client using the given topic prefixes. The optional topic_prefix should be of
    /// the form `my.custom.topic` with no trailing period. If a topic is provided and it does not
    /// have this format, an error will be returned. If `None` is passed as a topic prefix, the
    /// default topic prefix will be used.
    pub fn new_with_prefix(
        client: Client,
        user_topic_prefix: Option<String>,
        admin_topic_prefix: Option<String>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            client,
            user_topic_prefix: crate::sanitize_topic_prefix(
                user_topic_prefix,
                DEFAULT_USER_NATS_SUBJECT_PREFIX,
            )?,
            admin_topic_prefix: crate::sanitize_topic_prefix(
                admin_topic_prefix,
                DEFAULT_ADMIN_NATS_SUBJECT_PREFIX,
            )?,
        })
    }

    async fn do_request<T: Serialize, R: DeserializeOwned>(
        &self,
        subject: String,
        body: &T,
    ) -> anyhow::Result<GenericResponse<R>> {
        let serialized = serde_json::to_vec(body)?;
        let response = self.client.request(subject, serialized.into()).await?;
        serde_json::from_slice(&response.payload).context("unable to deserialize response")
    }
}

impl super::UserClient for NatsClient {
    async fn verify(
        &self,
        username: &str,
        password: SecureString,
    ) -> anyhow::Result<VerificationResponse> {
        let subject = format!("{}.verify", self.user_topic_prefix);
        let payload = VerificationRequest {
            username: username.to_string(),
            password,
        };
        let resp: GenericResponse<VerificationResponse> =
            self.do_request(subject, &payload).await?;
        resp.into_result_required()
            .context("Error while verifying user")
    }

    async fn change_password(
        &self,
        username: &str,
        old_password: SecureString,
        new_password: SecureString,
    ) -> anyhow::Result<()> {
        let subject = format!("{}.change_password", self.user_topic_prefix);
        let payload = PasswordChangeRequest {
            username: username.to_string(),
            old_password,
            new_password,
        };
        let resp: GenericResponse<()> = self.do_request(subject, &payload).await?;
        resp.into_result_empty()
            .context("Error while changing password")
    }
}

impl super::AdminClient for NatsClient {
    async fn add_user(
        &self,
        username: &str,
        password: SecureString,
        groups: BTreeSet<String>,
        force_password_change: bool,
    ) -> anyhow::Result<()> {
        let subject = format!("{}.add_user", self.admin_topic_prefix);
        let payload = UserAddRequest {
            username: username.to_string(),
            password,
            groups,
            force_password_change,
        };
        let resp: GenericResponse<()> = self.do_request(subject, &payload).await?;
        resp.into_result_empty().context("Error while adding user")
    }

    async fn get_user(&self, username: &str) -> anyhow::Result<UserResponse> {
        let subject = format!("{}.get_user", self.admin_topic_prefix);
        let payload = UserGetRequest {
            username: username.to_string(),
        };
        let resp: GenericResponse<UserResponse> = self.do_request(subject, &payload).await?;
        resp.into_result_required()
            .context("Error while getting user")
    }

    async fn list_users(&self) -> anyhow::Result<Vec<String>> {
        let subject = format!("{}.list_users", self.admin_topic_prefix);
        let resp: GenericResponse<Vec<String>> = self.do_request(subject, &()).await?;
        resp.into_result_required()
            .context("Error while listing users")
    }

    async fn remove_user(&self, username: &str) -> anyhow::Result<()> {
        let subject = format!("{}.remove_user", self.admin_topic_prefix);
        let payload = UserDeleteRequest {
            username: username.to_string(),
        };
        let resp: GenericResponse<()> = self.do_request(subject, &payload).await?;
        resp.into_result_empty()
            .context("Error while removing user")
    }

    async fn reset_password(&self, username: &str) -> anyhow::Result<PasswordResetResponse> {
        let subject = format!("{}.reset_password", self.admin_topic_prefix);
        let payload = PasswordResetRequest {
            username: username.to_string(),
        };
        let resp: GenericResponse<PasswordResetResponse> =
            self.do_request(subject, &payload).await?;
        resp.into_result_required()
            .context("Error while resetting password")
    }

    async fn add_groups(
        &self,
        username: &str,
        groups: BTreeSet<String>,
    ) -> anyhow::Result<BTreeSet<String>> {
        let subject = format!("{}.add_groups", self.admin_topic_prefix);
        let payload = GroupModifyRequest {
            username: username.to_string(),
            groups,
        };
        let resp: GenericResponse<BTreeSet<String>> = self.do_request(subject, &payload).await?;
        resp.into_result_required()
            .context("Error while adding groups")
    }

    async fn remove_groups(
        &self,
        username: &str,
        groups: BTreeSet<String>,
    ) -> anyhow::Result<BTreeSet<String>> {
        let subject = format!("{}.remove_groups", self.admin_topic_prefix);
        let payload = GroupModifyRequest {
            username: username.to_string(),
            groups,
        };
        let resp: GenericResponse<BTreeSet<String>> = self.do_request(subject, &payload).await?;
        resp.into_result_required()
            .context("Error while removing groups")
    }
}
