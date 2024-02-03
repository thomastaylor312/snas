use async_nats::{Client, Subscriber};
use futures::StreamExt;
use tracing::{instrument, trace, warn};

use crate::{handlers::Handlers, ADMIN_NATS_QUEUE, ADMIN_NATS_SUBJECT};

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
                    //todo
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
}
