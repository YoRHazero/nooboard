use crate::{Database, Error, Result};
use rusqlite::{OptionalExtension, params};

/// Text is deliberately omitted from Debug output.
#[derive(Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    pub id: i64,
    pub text: String,
    pub source: String,
    pub copied_at_ms: i64,
}
impl std::fmt::Debug for HistoryEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HistoryEntry")
            .field("id", &self.id)
            .field("bytes", &self.text.len())
            .field("source", &self.source)
            .field("copied_at_ms", &self.copied_at_ms)
            .finish()
    }
}
pub struct HistoryQuery<'a> {
    /// Literal, case-sensitive substring, including % and _.
    pub contains: &'a str,
    pub limit: u32,
    pub offset: u32,
}
impl Default for HistoryQuery<'_> {
    fn default() -> Self {
        Self {
            contains: "",
            limit: 100,
            offset: 0,
        }
    }
}
fn entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<HistoryEntry> {
    Ok(HistoryEntry {
        id: row.get(0)?,
        text: row.get(1)?,
        source: row.get(2)?,
        copied_at_ms: row.get(3)?,
    })
}
impl Database {
    /// Applies caller-selected deduplication and retention in one transaction.
    pub fn record_text(
        &mut self,
        text: &str,
        source: &str,
        now_ms: i64,
        max_entries: u32,
        oldest_ms: i64,
    ) -> Result<i64> {
        if max_entries == 0 || oldest_ms > now_ms {
            return Err(Error::InvalidArgument("invalid history retention"));
        }
        let tx = self.connection.transaction()?;
        let id = tx.query_row(
            "INSERT INTO history(text,source,copied_at_ms) VALUES(?1,?2,?3)
             ON CONFLICT(text) DO UPDATE SET source=excluded.source,copied_at_ms=excluded.copied_at_ms RETURNING id",
            params![text, source, now_ms], |row| row.get(0))?;
        tx.execute("DELETE FROM history WHERE copied_at_ms < ?1", [oldest_ms])?;
        tx.execute(
            "DELETE FROM history WHERE id IN (
            SELECT id FROM history ORDER BY copied_at_ms DESC,id DESC LIMIT -1 OFFSET ?1)",
            [max_entries],
        )?;
        tx.commit()?;
        Ok(id)
    }
    pub fn prune_history(&mut self, max_entries: u32, oldest_ms: i64) -> Result<()> {
        let tx = self.connection.transaction()?;
        tx.execute("DELETE FROM history WHERE copied_at_ms < ?1", [oldest_ms])?;
        tx.execute(
            "DELETE FROM history WHERE id IN (
            SELECT id FROM history ORDER BY copied_at_ms DESC,id DESC LIMIT -1 OFFSET ?1)",
            [max_entries],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn history(&self, query: HistoryQuery<'_>) -> Result<Vec<HistoryEntry>> {
        let mut statement = self.connection.prepare(
            "SELECT id,text,source,copied_at_ms FROM history WHERE instr(text,?1)>0
             ORDER BY copied_at_ms DESC,id DESC LIMIT ?2 OFFSET ?3",
        )?;
        Ok(statement
            .query_map(
                params![query.contains, query.limit.min(1000), query.offset],
                entry,
            )?
            .collect::<rusqlite::Result<_>>()?)
    }
    pub fn history_entry(&self, id: i64) -> Result<Option<HistoryEntry>> {
        Ok(self
            .connection
            .query_row(
                "SELECT id,text,source,copied_at_ms FROM history WHERE id=?1",
                [id],
                entry,
            )
            .optional()?)
    }
    pub fn delete_history(&mut self, id: i64) -> Result<bool> {
        Ok(self
            .connection
            .execute("DELETE FROM history WHERE id=?1", [id])?
            > 0)
    }
    pub fn clear_history(&mut self) -> Result<()> {
        self.connection.execute("DELETE FROM history", [])?;
        // This is not a secure erase guarantee.
        self.connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(())
    }
}
