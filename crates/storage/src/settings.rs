use crate::{Database, Result};
use rusqlite::{OptionalExtension, params};
impl Database {
    /// Stores opaque application configuration; never use this for private keys.
    pub fn set_setting(&mut self, key: &str, value: &[u8]) -> Result<()> {
        self.connection.execute(
            "INSERT INTO settings(key,value) VALUES(?1,?2)
            ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
    pub fn setting(&self, key: &str) -> Result<Option<Vec<u8>>> {
        Ok(self
            .connection
            .query_row("SELECT value FROM settings WHERE key=?1", [key], |r| {
                r.get(0)
            })
            .optional()?)
    }
    pub fn delete_setting(&mut self, key: &str) -> Result<()> {
        self.connection
            .execute("DELETE FROM settings WHERE key=?1", [key])?;
        Ok(())
    }
}
