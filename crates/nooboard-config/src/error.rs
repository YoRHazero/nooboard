use std::path::PathBuf;

use thiserror::Error;

pub type ConfigResult<T> = Result<T, ConfigError>;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse app config `{path}`: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("failed to serialize app config: {0}")]
    Serialize(#[from] toml::ser::Error),
    #[error("invalid app config: {0}")]
    InvalidConfig(String),
    #[error("invalid bootstrap request: {0}")]
    InvalidBootstrap(String),
}
