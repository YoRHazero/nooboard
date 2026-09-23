use crate::{Error, ErrorKind};

#[derive(Debug)]
pub(super) enum Failure {
    Database(rusqlite::Error),
    Io(std::io::Error),
    Corrupt(&'static str),
    SchemaTooNew,
}
impl From<rusqlite::Error> for Failure {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error)
    }
}
impl From<std::io::Error> for Failure {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
impl Failure {
    pub fn into_error(self, operation: &'static str) -> Error {
        match self {
            Self::SchemaTooNew => Error::new(ErrorKind::SchemaTooNew, operation),
            Self::Corrupt(reason) => Error::caused_by(
                ErrorKind::Corrupt,
                operation,
                std::io::Error::new(std::io::ErrorKind::InvalidData, reason),
            ),
            Self::Io(error) => {
                let kind = if error.kind() == std::io::ErrorKind::PermissionDenied {
                    ErrorKind::PermissionDenied
                } else {
                    ErrorKind::Unavailable
                };
                Error::caused_by(kind, operation, error)
            }
            Self::Database(error) => {
                use rusqlite::{Error as E, ErrorCode as C};
                let kind = match &error {
                    E::SqliteFailure(code, _) => match code.code {
                        C::DatabaseBusy | C::DatabaseLocked => ErrorKind::Busy,
                        C::PermissionDenied | C::ReadOnly => ErrorKind::PermissionDenied,
                        C::DatabaseCorrupt | C::NotADatabase => ErrorKind::Corrupt,
                        C::ConstraintViolation => ErrorKind::Conflict,
                        _ => ErrorKind::Unavailable,
                    },
                    E::FromSqlConversionFailure(..)
                    | E::InvalidColumnType(..)
                    | E::Utf8Error(..)
                    | E::IntegralValueOutOfRange(..) => ErrorKind::Corrupt,
                    _ => ErrorKind::Internal,
                };
                Error::caused_by(kind, operation, error)
            }
        }
    }
}
pub(super) type SqliteResult<T> = std::result::Result<T, Failure>;
