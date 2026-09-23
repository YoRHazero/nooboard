use super::error::{Failure, SqliteResult};
use crate::{
    HistoryEntry, HistoryId, HistoryPage, HistoryQuery, HistorySource, NewHistoryEntry,
    RecordHistory, RecordOutcome, Retention, SourceFilter,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};
use sha2::{Digest, Sha256};

pub(super) fn digest(text: &str) -> [u8; 32] {
    Sha256::digest(text.as_bytes()).into()
}
pub(super) fn source_columns(source: &HistorySource) -> (i64, Option<&str>) {
    match source {
        HistorySource::Local => (0, None),
        HistorySource::Remote(id) => (1, Some(id)),
    }
}
fn entry(row: &Row<'_>) -> rusqlite::Result<HistoryEntry> {
    let id: i64 = row.get(0)?;
    let id =
        HistoryId::try_from(id).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(0, id))?;
    let kind: i64 = row.get(2)?;
    let source_id: Option<String> = row.get(3)?;
    let source = match (kind, source_id) {
        (0, None) => HistorySource::Local,
        (1, Some(id)) => HistorySource::Remote(id),
        _ => {
            return Err(rusqlite::Error::FromSqlConversionFailure(
                2,
                rusqlite::types::Type::Integer,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "invalid history source",
                )),
            ));
        }
    };
    let entry = NewHistoryEntry {
        text: row.get(1)?,
        source,
        copied_at_ms: row.get(4)?,
    };
    entry.validate().map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(error))
    })?;
    Ok(HistoryEntry { id, entry })
}
pub(super) fn record(
    connection: &mut Connection,
    input: RecordHistory,
) -> SqliteResult<RecordOutcome> {
    let hash = digest(&input.entry.text);
    record_with_digest(connection, input, hash)
}
fn record_with_digest(
    connection: &mut Connection,
    input: RecordHistory,
    hash: [u8; 32],
) -> SqliteResult<RecordOutcome> {
    // Serialize lookup+insert across independent services sharing this database.
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let previous: Option<i64> = tx
        .query_row(
            "SELECT id FROM history WHERE text_hash=?1 AND text=?2 COLLATE BINARY",
            params![hash.as_slice(), input.entry.text],
            |row| row.get(0),
        )
        .optional()?;
    let (kind, source) = source_columns(&input.entry.source);
    let id = if let Some(id) = previous {
        tx.execute(
            "UPDATE history SET source_kind=?1,source_id=?2,copied_at_ms=?3 WHERE id=?4",
            params![kind, source, input.entry.copied_at_ms, id],
        )?;
        id
    } else {
        tx.query_row("INSERT INTO history(text,text_hash,source_kind,source_id,copied_at_ms) VALUES(?1,?2,?3,?4,?5) RETURNING id", params![input.entry.text, hash.as_slice(), kind, source, input.entry.copied_at_ms], |row| row.get(0))?
    };
    let id = HistoryId::try_from(id).map_err(|_| Failure::Corrupt("invalid history ID"))?;
    let pruned = prune_in(&tx, input.retention)?;
    let retained = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM history WHERE id=?1)",
        [id.value()],
        |row| row.get(0),
    )?;
    tx.commit()?;
    Ok(RecordOutcome {
        id,
        inserted: previous.is_none(),
        retained,
        pruned,
    })
}
fn prune_in(tx: &Transaction<'_>, retention: Retention) -> SqliteResult<u64> {
    let expired = tx.execute(
        "DELETE FROM history WHERE copied_at_ms < ?1",
        [retention.oldest_ms],
    )?;
    let excess = tx.execute("DELETE FROM history WHERE id IN (SELECT id FROM history ORDER BY copied_at_ms DESC,id DESC LIMIT -1 OFFSET ?1)", [retention.max_entries])?;
    Ok(expired as u64 + excess as u64)
}
pub(super) fn prune(connection: &mut Connection, retention: Retention) -> SqliteResult<u64> {
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let count = prune_in(&tx, retention)?;
    tx.commit()?;
    Ok(count)
}
pub(super) fn query(connection: &mut Connection, query: HistoryQuery) -> SqliteResult<HistoryPage> {
    let (kind, device) = match &query.source {
        SourceFilter::All => (None, None),
        SourceFilter::Local => (Some(0), None),
        SourceFilter::Remote => (Some(1), None),
        SourceFilter::Device(id) => (Some(1), Some(id.as_str())),
    };
    let mut statement = connection.prepare(
        "SELECT id,text,source_kind,source_id,copied_at_ms FROM history
        WHERE instr(text,?1)>0 AND (?2 IS NULL OR source_kind=?2) AND (?3 IS NULL OR source_id=?3)
        ORDER BY copied_at_ms DESC,id DESC LIMIT ?4 OFFSET ?5",
    )?;
    let mut entries = statement
        .query_map(
            params![
                query.contains,
                kind,
                device,
                query.limit + 1,
                query.offset as i64
            ],
            entry,
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let next_offset = if entries.len() > query.limit as usize {
        entries.pop();
        Some(query.offset + u64::from(query.limit))
    } else {
        None
    };
    Ok(HistoryPage {
        entries,
        next_offset,
    })
}
pub(super) fn get(
    connection: &mut Connection,
    id: HistoryId,
) -> SqliteResult<Option<HistoryEntry>> {
    Ok(connection
        .query_row(
            "SELECT id,text,source_kind,source_id,copied_at_ms FROM history WHERE id=?1",
            [id.value()],
            entry,
        )
        .optional()?)
}
pub(super) fn delete(connection: &mut Connection, id: HistoryId) -> SqliteResult<bool> {
    Ok(connection.execute("DELETE FROM history WHERE id=?1", [id.value()])? > 0)
}
pub(super) fn clear(connection: &mut Connection) -> SqliteResult<u64> {
    Ok(connection.execute("DELETE FROM history", [])? as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn digest_collision_never_merges_distinct_text() {
        let mut db = super::super::connection::open(crate::SqliteOptions::in_memory()).unwrap();
        let make = |text: &str| RecordHistory {
            entry: NewHistoryEntry {
                text: text.into(),
                source: HistorySource::Local,
                copied_at_ms: 1,
            },
            retention: Retention {
                max_entries: 10,
                oldest_ms: 0,
            },
        };
        let a = record_with_digest(&mut db, make("first"), [7; 32]).unwrap();
        let b = record_with_digest(&mut db, make("second"), [7; 32]).unwrap();
        let again = record_with_digest(&mut db, make("first"), [7; 32]).unwrap();
        assert_ne!(a.id, b.id);
        assert_eq!(a.id, again.id);
        assert!(!again.inserted);
    }
}
