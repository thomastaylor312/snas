use async_nats::{Client, Subject, Subscriber};
use futures::StreamExt;
use serde::Serialize;
use tracing::{error, instrument, trace, warn};

use crate::{
    api::{GenericResponse, VerificationRequest},
    handlers::Handlers,
    ADMIN_NATS_QUEUE, ADMIN_NATS_SUBJECT, USER_NATS_QUEUE, USER_NATS_SUBJECT,
};

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
                "add_user" => {
                    // TODO
                }
                "change_password" => {
                    // TODO
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

    async fn handle_verify(&self, msg: async_nats::Message) {
        let req: VerificationRequest = match serde_json::from_slice(&msg.payload) {
            Ok(req) => req,
            Err(err) => {
                send_error(
                    &self.client,
                    msg.reply,
                    format!("invalid request, unable to deserialize body: {}", err),
                )
                .await;
                return;
            }
        };
        match self.handlers.verify(&req.username, req.password).await {
            Ok(r) => {
                send_response(&self.client, msg.reply, r).await;
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
}

pub struct NatsAdminServer {
    handlers: Handlers,
    client: Client,
    subscription: Subscriber,
}

impl NatsAdminServer {
    pub async fn new(handlers: Handlers, client: Client) -> anyhow::Result<Self> {
        let subscription = client
            .queue_subscribe(ADMIN_NATS_SUBJECT, ADMIN_NATS_QUEUE.to_string())
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
            if split[1] != "admin" {
                warn!(subject = %msg.subject, "non-admin subject received");
            }
        }
        todo!()
    }
}

async fn send_error(client: &Client, reply: Option<Subject>, message: String) {
    if let Some(reply) = reply {
        if let Err(err) = client
            .publish(
                reply,
                serde_json::to_vec(&GenericResponse {
                    success: false,
                    message,
                })
                .expect("Unable to serialize generic response, this is likely programmer error")
                .into(),
            )
            .await
        {
            error!(%err, "unable to send error response");
        }
    }
}

async fn send_response<T: Serialize>(client: &Client, reply: Option<Subject>, response: T) {
    if let Some(reply) = reply {
        let body = match serde_json::to_vec(&response) {
            Ok(body) => body,
            Err(err) => {
                send_error(
                    client,
                    Some(reply),
                    format!("unable to serialize response: {}", err),
                )
                .await;
                return;
            }
        };
        if let Err(err) = client.publish(reply, body.into()).await {
            error!(%err, "unable to send response");
        }
    }
}
