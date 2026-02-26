pub mod actor;
pub mod outbox;
pub mod path;
pub mod receiver;
pub mod sender;

pub type SessionResult<T> = Result<T, crate::error::ConnectionError>;
