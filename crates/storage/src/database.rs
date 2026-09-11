use crate::{Error, Result};
use rusqlite::Connection;
use std::{path::Path, time::Duration};

/// Owns one SQLite connection; callers serialize access on their storage worker.
/// This database is plaintext. Use a private, per-user application directory.
pub struct Database {
    pub(crate) connection: Connection,
}
impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(path) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }
        Self::initialize(Connection::open(path)?)
    }
    pub fn in_memory() -> Result<Self> {
        Self::initialize(Connection::open_in_memory()?)
    }
    fn initialize(mut connection: Connection) -> Result<Self> {
        connection.busy_timeout(Duration::from_secs(2))?;
        let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version > 1 {
            return Err(Error::NewerSchema);
        }
        connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA secure_delete=ON;")?;
        let tx = connection.transaction()?;
        tx.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                text TEXT NOT NULL UNIQUE,
                source TEXT NOT NULL,
                copied_at_ms INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS history_recent ON history(copied_at_ms DESC, id DESC);
            CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value BLOB NOT NULL);
            PRAGMA user_version=1;
        ",
        )?;
        tx.commit()?;
        Ok(Self { connection })
    }
}
