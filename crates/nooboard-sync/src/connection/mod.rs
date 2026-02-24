pub mod actor;
pub mod path;
pub mod receiver;
pub mod sender;
pub mod stream;

pub type ConnectionResult<T> = Result<T, crate::error::ConnectionError>;
