pub mod clients;
pub mod error;
pub mod handlers;
pub mod servers;
pub mod storage;
pub mod types;

pub use types::*;

pub(crate) const ADMIN_NATS_SUBJECT_PREFIX: &str = "snas.admin.";
pub(crate) const ADMIN_NATS_QUEUE: &str = "snas_admin";
pub(crate) const USER_NATS_SUBJECT_PREFIX: &str = "snas.user.";
pub(crate) const USER_NATS_QUEUE: &str = "snas_user";
