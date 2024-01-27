use async_nats::{Client, Subscriber};
use futures::StreamExt;

use crate::{
    handlers::Handlers, ADMIN_NATS_QUEUE, ADMIN_NATS_SUBJECT, USER_NATS_QUEUE, USER_NATS_SUBJECT,
};

pub struct NatsUserServer {
    handlers: Handlers,
    subscription: Subscriber,
}

impl NatsUserServer {
    pub async fn new(handlers: Handlers, client: &Client) -> anyhow::Result<Self> {
        let subscription = client
            .queue_subscribe(USER_NATS_SUBJECT, USER_NATS_QUEUE.to_string())
            .await?;
        Ok(Self {
            handlers,
            subscription,
        })
    }

    pub async fn run(mut self) -> anyhow::Result<()> {
        while let Some(msg) = self.subscription.next().await {
            // TODO
        }
        todo!()
    }
}

pub struct NatsAdminServer {
    handlers: Handlers,
    subscription: Subscriber,
}

impl NatsAdminServer {
    pub async fn new(handlers: Handlers, client: &Client) -> anyhow::Result<Self> {
        let subscription = client
            .queue_subscribe(ADMIN_NATS_SUBJECT, ADMIN_NATS_QUEUE.to_string())
            .await?;
        Ok(Self {
            handlers,
            subscription,
        })
    }

    pub async fn run(mut self) -> anyhow::Result<()> {
        while let Some(msg) = self.subscription.next().await {
            // TODO
        }
        todo!()
    }
}
