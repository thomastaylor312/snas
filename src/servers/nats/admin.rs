use async_nats::{Client, Message, Subscriber};
use futures::StreamExt;
use tracing::{instrument, trace, warn};

use crate::{
    admin::{
        AdminUserAddRequest, GroupModifyRequest, PasswordResetRequest, UserApproveRequest,
        UserDeleteRequest, UserGetRequest,
    },
    handlers::Handlers,
    ADMIN_NATS_QUEUE, ADMIN_NATS_SUBJECT,
};

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
                    self.handle_approve_user(msg).await;
                }
                "get_user" => {
                    self.handle_get_user(msg).await;
                }
                "list_users" => {
                    self.handle_list_users(msg).await;
                }
                "remove_user" => {
                    self.handle_remove_user(msg).await;
                }
                "reset_password" => {
                    self.handle_reset_password(msg).await;
                }
                "add_groups" => {
                    self.handle_add_groups(msg).await;
                }
                "remove_groups" => {
                    self.handle_delete_groups(msg).await;
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

        let username = req.username.clone();
        match self.handlers.add(req).await {
            Ok(_) => {
                send_response(
                    &self.client,
                    msg.reply,
                    GenericResponse::new(true, format!("User {username} added")),
                )
                .await;
            }
            Err(e) => {
                send_error(&self.client, msg.reply, format!("Unable to add user: {e}")).await;
            }
        }
    }

    async fn handle_approve_user(&self, msg: Message) {
        let req =
            deserialize_body::<UserApproveRequest>(&self.client, &msg.payload, msg.reply.as_ref())
                .await;
        if req.is_err() {
            // deserialize_body sends the error back for us so we can just return
            return;
        }
        let req = req.unwrap();

        match self.handlers.set_approval(&req.username, req.approve).await {
            Ok(_) => {
                send_response(
                    &self.client,
                    msg.reply,
                    GenericResponse::new(
                        true,
                        format!(
                            "User {} approval status set to {}",
                            req.username, req.approve
                        ),
                    ),
                )
                .await;
            }
            Err(e) => {
                send_error(
                    &self.client,
                    msg.reply,
                    format!("Unable to approve user: {e}"),
                )
                .await;
            }
        }
    }

    async fn handle_get_user(&self, msg: Message) {
        let req =
            deserialize_body::<UserGetRequest>(&self.client, &msg.payload, msg.reply.as_ref())
                .await;
        if req.is_err() {
            // deserialize_body sends the error back for us so we can just return
            return;
        }
        let req = req.unwrap();

        match self.handlers.get(&req.username).await {
            Ok(user) => {
                send_response(
                    &self.client,
                    msg.reply,
                    GenericResponse {
                        success: true,
                        message: String::new(),
                        response: Some(user),
                    },
                )
                .await;
            }
            Err(e) => {
                send_error(&self.client, msg.reply, format!("Unable to get user: {e}")).await;
            }
        }
    }

    async fn handle_list_users(&self, msg: Message) {
        // We don't need to parse a body as we are listing all usernames

        match self.handlers.list().await {
            Ok(users) => {
                send_response(
                    &self.client,
                    msg.reply,
                    GenericResponse {
                        success: true,
                        message: String::new(),
                        response: Some(users),
                    },
                )
                .await;
            }
            Err(e) => {
                send_error(
                    &self.client,
                    msg.reply,
                    format!("Unable to list users: {e}"),
                )
                .await;
            }
        }
    }

    async fn handle_remove_user(&self, msg: Message) {
        let req =
            deserialize_body::<UserDeleteRequest>(&self.client, &msg.payload, msg.reply.as_ref())
                .await;
        if req.is_err() {
            // deserialize_body sends the error back for us so we can just return
            return;
        }
        let req = req.unwrap();

        match self.handlers.delete(&req.username).await {
            Ok(_) => {
                send_response(
                    &self.client,
                    msg.reply,
                    GenericResponse::new(true, format!("User {} deleted", req.username)),
                )
                .await;
            }
            Err(e) => {
                send_error(
                    &self.client,
                    msg.reply,
                    format!("Unable to remove user: {e}"),
                )
                .await;
            }
        }
    }

    async fn handle_reset_password(&self, msg: Message) {
        let req = deserialize_body::<PasswordResetRequest>(
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

        match self.handlers.reset_password(&req.username).await {
            Ok(resp) => {
                send_response(
                    &self.client,
                    msg.reply,
                    GenericResponse {
                        success: true,
                        message: format!("Password reset for user {}", req.username),
                        response: Some(resp),
                    },
                )
                .await;
            }
            Err(e) => {
                send_error(
                    &self.client,
                    msg.reply,
                    format!("Unable to reset password for user: {e}"),
                )
                .await;
            }
        }
    }

    async fn handle_add_groups(&self, msg: Message) {
        let req =
            deserialize_body::<GroupModifyRequest>(&self.client, &msg.payload, msg.reply.as_ref())
                .await;
        if req.is_err() {
            // deserialize_body sends the error back for us so we can just return
            return;
        }
        let req = req.unwrap();

        match self.handlers.add_groups(&req.username, req.groups).await {
            Ok(resp) => {
                send_response(
                    &self.client,
                    msg.reply,
                    GenericResponse {
                        success: true,
                        message: format!("Updated groups for user {}", req.username),
                        response: Some(resp),
                    },
                )
                .await;
            }
            Err(e) => {
                send_error(
                    &self.client,
                    msg.reply,
                    format!("Unable to add groups for user: {e}"),
                )
                .await;
            }
        }
    }

    async fn handle_delete_groups(&self, msg: Message) {
        let req =
            deserialize_body::<GroupModifyRequest>(&self.client, &msg.payload, msg.reply.as_ref())
                .await;
        if req.is_err() {
            // deserialize_body sends the error back for us so we can just return
            return;
        }
        let req = req.unwrap();

        match self.handlers.add_groups(&req.username, req.groups).await {
            Ok(resp) => {
                send_response(
                    &self.client,
                    msg.reply,
                    GenericResponse {
                        success: true,
                        message: format!("Deleted groups from user {}", req.username),
                        response: Some(resp),
                    },
                )
                .await;
            }
            Err(e) => {
                send_error(
                    &self.client,
                    msg.reply,
                    format!("Unable to delete groups for user: {e}"),
                )
                .await;
            }
        }
    }
}
