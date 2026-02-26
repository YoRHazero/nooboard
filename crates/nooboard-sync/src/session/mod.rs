pub mod actor;
pub mod path;
pub mod receiver;
pub mod sender;
pub mod outbox;

pub type SessionResult<T> = Result<T, crate::error::ConnectionError>;
