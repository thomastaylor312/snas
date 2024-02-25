pub mod clients;
pub mod error;
pub mod handlers;
pub mod servers;
pub mod storage;
pub mod types;

pub use types::*;

pub(crate) const DEFAULT_ADMIN_NATS_SUBJECT_PREFIX: &str = "snas.admin";
pub(crate) const DEFAULT_USER_NATS_SUBJECT_PREFIX: &str = "snas.user";
