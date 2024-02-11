use std::collections::BTreeSet;

use anyhow::Context;
use async_nats::Client;
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    admin::{
        GroupModifyRequest, PasswordResetRequest, PasswordResetResponse, UserAddRequest,
        UserDeleteRequest, UserGetRequest, UserResponse,
    },
    api::{
        EmptyResponse, GenericResponse, PasswordChangeRequest, VerificationRequest,
        VerificationResponse,
    },
    SecureString, ADMIN_NATS_SUBJECT_PREFIX, USER_NATS_SUBJECT_PREFIX,
};

pub struct NatsClient {
    client: Client,
}

impl NatsClient {
    pub fn new(client: Client) -> Self {
        Self { client }
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
        let subject = format!("{USER_NATS_SUBJECT_PREFIX}.verify");
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
        let subject = format!("{USER_NATS_SUBJECT_PREFIX}.change_password");
        let payload = PasswordChangeRequest {
            username: username.to_string(),
            old_password,
            new_password,
        };
        let resp: GenericResponse<EmptyResponse> = self.do_request(subject, &payload).await?;
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
        let subject = format!("{ADMIN_NATS_SUBJECT_PREFIX}.add_user");
        let payload = UserAddRequest {
            username: username.to_string(),
            password,
            groups,
            force_password_change,
        };
        let resp: GenericResponse<EmptyResponse> = self.do_request(subject, &payload).await?;
        resp.into_result_empty().context("Error while adding user")
    }

    async fn get_user(&self, username: &str) -> anyhow::Result<UserResponse> {
        let subject = format!("{ADMIN_NATS_SUBJECT_PREFIX}.get_user");
        let payload = UserGetRequest {
            username: username.to_string(),
        };
        let resp: GenericResponse<UserResponse> = self.do_request(subject, &payload).await?;
        resp.into_result_required()
            .context("Error while getting user")
    }

    async fn list_users(&self) -> anyhow::Result<Vec<String>> {
        let subject = format!("{ADMIN_NATS_SUBJECT_PREFIX}.list_users");
        let resp: GenericResponse<Vec<String>> = self.do_request(subject, &()).await?;
        resp.into_result_required()
            .context("Error while listing users")
    }

    async fn remove_user(&self, username: &str) -> anyhow::Result<()> {
        let subject = format!("{ADMIN_NATS_SUBJECT_PREFIX}.remove_user");
        let payload = UserDeleteRequest {
            username: username.to_string(),
        };
        let resp: GenericResponse<EmptyResponse> = self.do_request(subject, &payload).await?;
        resp.into_result_empty()
            .context("Error while removing user")
    }

    async fn reset_password(&self, username: &str) -> anyhow::Result<PasswordResetResponse> {
        let subject = format!("{ADMIN_NATS_SUBJECT_PREFIX}.reset_password");
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
        let subject = format!("{ADMIN_NATS_SUBJECT_PREFIX}.add_groups");
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
        let subject = format!("{ADMIN_NATS_SUBJECT_PREFIX}.remove_groups");
        let payload = GroupModifyRequest {
            username: username.to_string(),
            groups,
        };
        let resp: GenericResponse<BTreeSet<String>> = self.do_request(subject, &payload).await?;
        resp.into_result_required()
            .context("Error while removing groups")
    }
}
