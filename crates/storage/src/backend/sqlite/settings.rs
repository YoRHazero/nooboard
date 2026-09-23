use super::error::SqliteResult;
use crate::SettingsChanges;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

pub(super) fn read(
    connection: &mut Connection,
    keys: Vec<String>,
) -> SqliteResult<Vec<Option<String>>> {
    let tx = connection.transaction()?;
    let values = {
        let mut statement = tx.prepare("SELECT value FROM settings WHERE key=?1")?;
        keys.into_iter()
            .map(|key| statement.query_row([key], |row| row.get(0)).optional())
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    tx.commit()?;
    Ok(values)
}
pub(super) fn apply(connection: &mut Connection, changes: SettingsChanges) -> SqliteResult<()> {
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    for setting in changes.put {
        tx.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![setting.key, setting.value])?;
    }
    for key in changes.delete {
        tx.execute("DELETE FROM settings WHERE key=?1", [key])?;
    }
    tx.commit()?;
    Ok(())
}
