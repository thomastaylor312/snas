use async_nats::{Client, Message, Subscriber};
use futures::StreamExt;
use tracing::{instrument, trace, warn};

use crate::{
    api::{GenericResponse, PasswordChangeRequest, VerificationRequest, VerificationResponse},
    error::HandleError,
    handlers::Handlers,
    DEFAULT_USER_NATS_SUBJECT_PREFIX,
};

use super::*;

pub struct NatsUserServer {
    handlers: Handlers,
    client: Client,
    subscription: Subscriber,
    prefix: String,
}

impl NatsUserServer {
    /// Creates a new admin server. The optional topic_prefix should be of the form
    /// `my.custom.topic` with no trailing period. If a topic is provided and it does not have this
    /// format, an error will be returned.
    ///
    /// If also running an admin server, this topic prefix MUST be different from the admin server's
    pub async fn new(
        handlers: Handlers,
        client: Client,
        topic_prefix: Option<String>,
    ) -> anyhow::Result<Self> {
        let subject_prefix =
            crate::sanitize_topic_prefix(topic_prefix, DEFAULT_USER_NATS_SUBJECT_PREFIX)?;
        let subscription = client
            .queue_subscribe(format!("{subject_prefix}.*"), subject_prefix.clone())
            .await?;
        Ok(Self {
            handlers,
            subscription,
            client,
            prefix: subject_prefix,
        })
    }

    #[instrument(level = "info", skip(self))]
    pub async fn run(mut self) -> anyhow::Result<()> {
        while let Some(msg) = self.subscription.next().await {
            let action = match msg.subject.strip_prefix(&self.prefix) {
                Some(a) => a.trim_start_matches('.'),
                None => {
                    warn!(subject = %msg.subject, "invalid subject received");
                    send_error(
                        &self.client,
                        msg.reply,
                        format!("invalid subject {}", msg.subject),
                    )
                    .await;
                    continue;
                }
            };
            match action {
                "change_password" => {
                    self.handle_change_password(msg).await;
                }
                "verify" => {
                    self.handle_verify(msg).await;
                }
                _ => {
                    trace!(subject = %msg.subject, "invalid subject received");
                    send_error(
                        &self.client,
                        msg.reply,
                        format!("invalid api method {}", action),
                    )
                    .await;
                }
            }
        }
        Err(anyhow::anyhow!("nats user server exited"))
    }

    #[instrument(level = "debug", skip_all, fields(subject = %msg.subject))]
    async fn handle_verify(&self, msg: Message) {
        let req =
            deserialize_body::<VerificationRequest>(&self.client, &msg.payload, msg.reply.as_ref())
                .await;
        if req.is_err() {
            // deserialize_body sends the error back for us so we can just return
            return;
        }
        let req = req.unwrap();
        match self.handlers.verify(&req.username, req.password).await {
            Ok(r) => {
                send_response(
                    &self.client,
                    msg.reply,
                    GenericResponse {
                        success: true,
                        message: "Verification succeeded".to_string(),
                        response: Some(r),
                    },
                )
                .await;
            }
            Err(HandleError::InvalidCredentials) => {
                send_response(
                    &self.client,
                    msg.reply,
                    GenericResponse {
                        success: true,
                        message: "Verification failed".to_string(),
                        response: Some(VerificationResponse {
                            valid: false,
                            message: HandleError::InvalidCredentials.to_string(),
                            needs_password_reset: false,
                            groups: Default::default(),
                        }),
                    },
                )
                .await;
            }
            Err(HandleError::PasswordResetExpired) => {
                send_response(
                    &self.client,
                    msg.reply,
                    GenericResponse {
                        success: true,
                        message: "Verification failed".to_string(),
                        response: Some(VerificationResponse {
                            valid: false,
                            message: HandleError::PasswordResetExpired.to_string(),
                            needs_password_reset: true,
                            groups: Default::default(),
                        }),
                    },
                )
                .await;
            }
            Err(err) => {
                send_error(
                    &self.client,
                    msg.reply,
                    format!("verification failed: {}", err),
                )
                .await;
            }
        }
    }

    #[instrument(level = "debug", skip_all, fields(subject = %msg.subject))]
    async fn handle_change_password(&self, msg: Message) {
        let req = deserialize_body::<PasswordChangeRequest>(
            &self.client,
            &msg.payload,
            msg.reply.as_ref(),
        )
        .await;
        if req.is_err() {
            // deserialize_body sends the error back for us so we can just return
            return;
        }
        let req = req.unwrap();

        match self
            .handlers
            .change_password(&req.username, req.old_password, req.new_password)
            .await
        {
            Ok(_) => {
                send_response(
                    &self.client,
                    msg.reply,
                    GenericResponse::new(true, "password changed".to_string()),
                )
                .await;
            }
            Err(err) => {
                send_error(
                    &self.client,
                    msg.reply,
                    format!("password change failed: {}", err),
                )
                .await;
            }
        }
    }
}
