use crate::{Database, Result};
use rusqlite::{OptionalExtension, params};
impl Database {
    /// Atomically replaces a configuration and retires its legacy keys.
    pub fn replace_settings(&mut self, key: &str, value: &[u8], obsolete: &[&str]) -> Result<()> {
        let tx = self.connection.transaction()?;
        tx.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key, value])?;
        for old in obsolete {
            if *old != key {
                tx.execute("DELETE FROM settings WHERE key=?1", [old])?;
            }
        }
        tx.commit()?;
        Ok(())
    }
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
