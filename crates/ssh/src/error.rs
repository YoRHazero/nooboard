use serde::Serialize;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error, Serialize)]
pub enum Error {
    #[error("SSH service stopped")]
    Stopped,
    #[error("operation cancelled")]
    Cancelled,
    #[error("connection or request timed out")]
    Timeout,
    #[error("SSH authentication failed; password authentication is required")]
    Authentication,
    #[error("SSH host key does not match the trusted key")]
    HostKeyChanged,
    #[error("SSH host key was not approved")]
    HostKeyRejected,
    #[error("SSH connection failed: {0}")]
    Connection(String),
    #[error("SFTP subsystem is unavailable")]
    SftpUnavailable,
    #[error("invalid path")]
    InvalidPath,
    #[error("path does not exist")]
    NotFound,
    #[error("permission denied")]
    PermissionDenied,
    #[error("path is not a directory")]
    NotDirectory,
    #[error("only ordinary files are supported")]
    NotFile,
    #[error("directory cursor expired; reload the directory")]
    StaleCursor,
    #[error("operation already in progress")]
    Busy,
    #[error("resource limit reached")]
    Limit,
    #[error("source file changed during transfer")]
    SourceChanged,
    #[error("publication result is unknown; check the target before retrying")]
    Unconfirmed,
    #[error("system credential store is unavailable")]
    Credentials,
    #[error("file operation failed: {0}")]
    Io(String),
    #[error("SFTP operation failed: {0}")]
    Protocol(String),
    #[error("SSH worker failed")]
    Internal,
}
impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        match value.kind() {
            std::io::ErrorKind::NotFound => Self::NotFound,
            std::io::ErrorKind::PermissionDenied => Self::PermissionDenied,
            _ => Self::Io(value.to_string()),
        }
    }
}
impl From<russh::Error> for Error {
    fn from(value: russh::Error) -> Self {
        Self::Connection(value.to_string())
    }
}
impl From<russh_sftp::client::error::Error> for Error {
    fn from(value: russh_sftp::client::error::Error) -> Self {
        use russh_sftp::{client::error::Error as E, protocol::StatusCode};
        match value {
            E::Timeout => Self::Timeout,
            E::Status(s) if s.status_code == StatusCode::NoSuchFile => Self::NotFound,
            E::Status(s) if s.status_code == StatusCode::PermissionDenied => Self::PermissionDenied,
            other => Self::Protocol(other.to_string()),
        }
    }
}
