//! SQLite persistence and native secret storage. Business policies belong to core.
mod database;
mod history;
pub mod secrets;
mod settings;
pub use database::Database;
pub use history::{HistoryEntry, HistoryQuery};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database operation failed")]
    Database(#[from] rusqlite::Error),
    #[error("filesystem operation failed")]
    Io(#[from] std::io::Error),
    #[error("database schema is newer than this application")]
    NewerSchema,
    #[error("invalid storage argument: {0}")]
    InvalidArgument(&'static str),
}
pub type Result<T> = std::result::Result<T, Error>;
