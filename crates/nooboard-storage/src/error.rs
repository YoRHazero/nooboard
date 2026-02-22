use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("failed to parse config file `{path}`: {source}")]
    ConfigParse {
        path: std::path::PathBuf,
        source: toml::de::Error,
    },
    #[error("limit is too large for SQLite: {0}")]
    LimitOutOfRange(usize),
    #[error("origin_seq is too large for SQLite INTEGER")]
    SeqOutOfRange,
}
