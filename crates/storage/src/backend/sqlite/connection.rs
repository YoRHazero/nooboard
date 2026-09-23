use super::{
    error::{Failure, SqliteResult},
    migration,
};
use crate::{SqliteLocation, SqliteOptions};
use rusqlite::{Connection, OpenFlags};

pub(super) fn open(options: SqliteOptions) -> SqliteResult<Connection> {
    let mut connection = match options.location {
        SqliteLocation::Memory => Connection::open_in_memory()?,
        SqliteLocation::File(path) => {
            // An absolute, non-URI path avoids treating a filename such as
            // ':memory:' as a special SQLite connection string.
            let path = if path.is_absolute() {
                path
            } else {
                std::env::current_dir()?.join(path)
            };
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut file = std::fs::OpenOptions::new();
            file.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                file.mode(0o600);
            }
            match file.open(&path) {
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
            Connection::open_with_flags(
                path,
                OpenFlags::SQLITE_OPEN_READ_WRITE
                    | OpenFlags::SQLITE_OPEN_CREATE
                    | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?
        }
    };
    connection.busy_timeout(options.busy_timeout)?;
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version > migration::VERSION {
        return Err(Failure::SchemaTooNew);
    }
    connection.execute_batch(
        "PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA secure_delete=ON;",
    )?;
    migration::migrate(&mut connection)?;
    Ok(connection)
}
