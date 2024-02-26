pub mod clients;
pub mod error;
pub mod handlers;
pub mod servers;
pub mod storage;
pub mod types;

pub use types::*;

pub(crate) const DEFAULT_ADMIN_NATS_SUBJECT_PREFIX: &str = "snas.admin";
pub(crate) const DEFAULT_USER_NATS_SUBJECT_PREFIX: &str = "snas.user";

pub(crate) fn sanitize_topic_prefix(
    prefix: Option<String>,
    default_prefix: &str,
) -> anyhow::Result<String> {
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
