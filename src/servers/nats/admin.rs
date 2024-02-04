use async_nats::{Client, Message, Subscriber};
use futures::StreamExt;
use tracing::{instrument, trace, warn};

use crate::{admin::AdminUserAddRequest, handlers::Handlers, ADMIN_NATS_QUEUE, ADMIN_NATS_SUBJECT};

use super::*;

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
            match split[2] {
                "add_user" => {
                    self.handle_add_user(msg).await;
                }
                "approve_user" => {
                    // todo
                }
                "get_user" => {
                    //todo
                }
                "list_users" => {
                    //todo
                }
                "remove_user" => {
                    //todo
                }
                "reset_password" => {
                    //todo
                }
                "add_groups" => {
                    //todo
                }
                "remove_groups" => {
                    //todo
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
        Err(anyhow::anyhow!("nats admin server exited"))
    }

    async fn handle_add_user(&self, msg: Message) {
        let req =
            deserialize_body::<AdminUserAddRequest>(&self.client, &msg.payload, msg.reply.as_ref())
                .await;
        if req.is_err() {
            // deserialize_body sends the error back for us so we can just return
            return;
        }
        let req = req.unwrap();

        match self
            .handlers
            .add(
                &req.username,
                req.password,
                false,
                req.force_password_change,
            )
            .await
        {
            Ok(_) => {
                send_response(
                    &self.client,
                    msg.reply,
                    GenericResponse::new(true, format!("User {} added", req.username)),
                )
                .await;
            }
            Err(e) => {
                send_error(&self.client, msg.reply, format!("Unable to add user: {e}")).await;
            }
        }
    }
}
