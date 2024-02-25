use async_nats::{Client, Subject};
use serde::{de::DeserializeOwned, Serialize};
use tracing::error;

use crate::types::api::{EmptyResponse, GenericResponse};

pub mod admin;
pub mod user;

fn sanitize_topic_prefix(prefix: Option<String>, default_prefix: &str) -> anyhow::Result<String> {
    match prefix {
        Some(prefix) => {
            let trimmed = prefix.trim();
            if trimmed.ends_with('.') {
                return Err(anyhow::anyhow!(
                    "topic_prefix must not end with a period, e.g. my.custom.topic"
                ));
            }
            Ok(trimmed.to_string())
        }
        None => Ok(default_prefix.to_string()),
    }
}

async fn send_error(client: &Client, reply: Option<Subject>, message: String) {
    if let Some(reply) = reply {
        if let Err(err) = client
            .publish(
                reply,
                serde_json::to_vec(&GenericResponse::<EmptyResponse> {
                    success: false,
                    message,
                    response: None,
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

async fn send_response<T: Serialize>(
    client: &Client,
    reply: Option<Subject>,
    response: GenericResponse<T>,
) {
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

async fn deserialize_body<T: DeserializeOwned>(
    client: &Client,
    body: &[u8],
    reply: Option<&Subject>,
) -> anyhow::Result<T> {
    match serde_json::from_slice(body) {
        Ok(body) => Ok(body),
        Err(err) => {
            send_error(
                client,
                reply.cloned(),
                format!("invalid request, unable to deserialize body: {}", err),
            )
            .await;
            Err(anyhow::anyhow!(
                "invalid request, unable to deserialize body"
            ))
        }
    }
}
