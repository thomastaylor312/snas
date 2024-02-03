use async_nats::{Client, Subscriber};
use futures::StreamExt;
use tracing::{instrument, trace, warn};

use crate::{
    api::{GenericResponse, PasswordChangeRequest, VerificationRequest},
    handlers::Handlers,
    USER_NATS_QUEUE, USER_NATS_SUBJECT,
};

use super::*;

pub struct NatsUserServer {
    handlers: Handlers,
    client: Client,
    subscription: Subscriber,
}

impl NatsUserServer {
    pub async fn new(handlers: Handlers, client: Client) -> anyhow::Result<Self> {
        let subscription = client
            .queue_subscribe(USER_NATS_SUBJECT, USER_NATS_QUEUE.to_string())
            .await?;
        Ok(Self {
            handlers,
            subscription,
            client,
        })
    }

    #[instrument(level = "info", skip(self))]
    pub async fn run(mut self) -> anyhow::Result<()> {
        while let Some(msg) = self.subscription.next().await {
            let split: Vec<&str> = msg.subject.split('.').collect();
            if split.len() != 3 {
                warn!(subject = %msg.subject, "invalid subject received");
                continue;
            }
            if split[1] != "user" {
                warn!(subject = %msg.subject, "non-user subject received");
                continue;
            }
            match split[2] {
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
                        format!("invalid api method {}", split[2]),
                    )
                    .await;
                }
            }
        }
        Err(anyhow::anyhow!("nats user server exited"))
    }

    #[instrument(level = "debug", skip_all, fields(subject = %msg.subject))]
    async fn handle_verify(&self, msg: async_nats::Message) {
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
    async fn handle_change_password(&self, msg: async_nats::Message) {
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
