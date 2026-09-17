//! SQLite persistence and native secret storage. Business policies belong to core.
mod database;
pub mod files;
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
    #[error("transfer cancelled")]
    Cancelled,
    #[error("transfer exceeds size or item limits")]
    TooLarge,
    #[error("source file changed while preparing the transfer")]
    SourceChanged,
    #[error("received file size or checksum does not match")]
    Integrity,
}
pub type Result<T> = std::result::Result<T, Error>;
