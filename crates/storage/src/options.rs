use crate::{Error, Result};
#[cfg(feature = "sqlite")]
use std::{path::PathBuf, time::Duration};

#[derive(Clone, Debug)]
pub struct Options {
    /// Pending requests; at most one additional request is executing.
    pub queue_capacity: usize,
}
impl Default for Options {
    fn default() -> Self {
        Self { queue_capacity: 32 }
    }
}
impl Options {
    pub(crate) fn validate(&self) -> Result<()> {
        if !(1..=1024).contains(&self.queue_capacity) {
            return Err(Error::invalid("queue capacity"));
        }
        Ok(())
    }
}
/// Backend selection belongs only to startup, never to individual requests.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum BackendConfig {
    #[cfg(feature = "sqlite")]
    Sqlite(SqliteOptions),
}
#[cfg(feature = "sqlite")]
#[derive(Clone, Debug)]
pub enum SqliteLocation {
    File(PathBuf),
    Memory,
}
#[cfg(feature = "sqlite")]
#[derive(Clone, Debug)]
pub struct SqliteOptions {
    pub location: SqliteLocation,
    pub busy_timeout: Duration,
}
#[cfg(feature = "sqlite")]
impl SqliteOptions {
    pub fn file(path: impl Into<PathBuf>) -> Self {
        Self {
            location: SqliteLocation::File(path.into()),
            busy_timeout: Duration::from_secs(2),
        }
    }
    pub fn in_memory() -> Self {
        Self {
            location: SqliteLocation::Memory,
            busy_timeout: Duration::from_secs(2),
        }
    }
    pub(crate) fn validate(&self) -> Result<()> {
        if self.busy_timeout > Duration::from_secs(30)
            || matches!(&self.location, SqliteLocation::File(path) if path.as_os_str().is_empty())
        {
            return Err(Error::invalid("SQLite options"));
        }
        Ok(())
    }
}
