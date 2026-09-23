use super::{
    error::{Failure, SqliteResult},
    history::{digest, source_columns},
};
use crate::{HistoryId, HistorySource, NewHistoryEntry, Setting, SettingsChanges};
use rusqlite::{Connection, TransactionBehavior, params};

pub(super) const VERSION: i64 = 2;
pub(super) fn migrate(connection: &mut Connection) -> SqliteResult<()> {
    // Re-read under a write lock: simultaneous first opens cannot both migrate.
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: i64 = tx.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version > VERSION {
        return Err(Failure::SchemaTooNew);
    }
    if version == VERSION {
        tx.commit()?;
        return Ok(());
    }
    if version != 0 && version != 1 {
        return Err(Failure::Corrupt("invalid schema version"));
    }
    if version == 1 {
        tx.execute_batch(
            "ALTER TABLE history RENAME TO legacy_history;
            ALTER TABLE settings RENAME TO legacy_settings;
            DROP INDEX IF EXISTS history_recent;",
        )?;
    }
    tx.execute_batch(include_str!("migrations/0002_text_storage.sql"))?;
    if version == 1 {
        {
            let mut statement =
                tx.prepare("SELECT id,text,source,copied_at_ms FROM legacy_history ORDER BY id")?;
            let mut rows = statement.query([])?;
            while let Some(row) = rows.next()? {
                let id: i64 = row.get(0)?;
                HistoryId::try_from(id)
                    .map_err(|_| Failure::Corrupt("invalid legacy history ID"))?;
                let source: String = row.get(2)?;
                let entry = NewHistoryEntry {
                    text: row.get(1)?,
                    source: if source == "local" {
                        HistorySource::Local
                    } else {
                        HistorySource::Remote(source)
                    },
                    copied_at_ms: row.get(3)?,
                };
                entry
                    .validate()
                    .map_err(|_| Failure::Corrupt("invalid legacy history entry"))?;
                let (kind, source) = source_columns(&entry.source);
                tx.execute("INSERT INTO history(id,text,text_hash,source_kind,source_id,copied_at_ms) VALUES(?1,?2,?3,?4,?5,?6)",
                    params![id, entry.text, digest(&entry.text).as_slice(), kind, source, entry.copied_at_ms])?;
            }
        }
        {
            let mut statement = tx.prepare("SELECT key,value FROM legacy_settings")?;
            let mut rows = statement.query([])?;
            while let Some(row) = rows.next()? {
                let key: String = row.get(0)?;
                let bytes = row
                    .get_ref(1)?
                    .as_bytes()
                    .map_err(|_| Failure::Corrupt("legacy setting is not text or bytes"))?;
                let value = std::str::from_utf8(bytes)
                    .map_err(|_| Failure::Corrupt("legacy setting is not UTF-8"))?
                    .to_owned();
                let changes = SettingsChanges {
                    put: vec![Setting { key, value }],
                    delete: vec![],
                };
                changes
                    .validate()
                    .map_err(|_| Failure::Corrupt("invalid legacy setting"))?;
                let setting = &changes.put[0];
                tx.execute(
                    "INSERT INTO settings(key,value) VALUES(?1,?2)",
                    params![setting.key, setting.value],
                )?;
            }
        }
        // Preserve the allocation watermark even if the newest old rows were deleted.
        tx.execute("UPDATE sqlite_sequence SET seq = MAX(seq, COALESCE((SELECT seq FROM sqlite_sequence WHERE name='legacy_history'),0)) WHERE name='history'", [])?;
        tx.execute("INSERT INTO sqlite_sequence(name,seq) SELECT 'history',seq FROM sqlite_sequence WHERE name='legacy_history' AND NOT EXISTS(SELECT 1 FROM sqlite_sequence WHERE name='history')", [])?;
        tx.execute_batch("DROP TABLE legacy_history; DROP TABLE legacy_settings;")?;
    }
    tx.pragma_update(None, "user_version", VERSION)?;
    tx.commit()?;
    Ok(())
}
