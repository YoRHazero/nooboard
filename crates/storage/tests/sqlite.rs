#![cfg(feature = "sqlite")]
mod contract;
use nooboard_storage::*;
use rusqlite::Connection;
use std::{path::Path, time::Duration};

async fn open(path: &Path) -> (StorageService, Storage) {
    StorageService::start(
        BackendConfig::Sqlite(SqliteOptions::file(path)),
        Options::default(),
    )
    .await
    .unwrap()
}
fn change(key: &str, value: &str) -> SettingsChanges {
    SettingsChanges {
        put: vec![Setting {
            key: key.into(),
            value: value.into(),
        }],
        delete: vec![],
    }
}

#[tokio::test]
async fn sqlite_satisfies_shared_contract_in_memory_and_on_disk() {
    let (service, storage) = StorageService::start(
        BackendConfig::Sqlite(SqliteOptions::in_memory()),
        Options::default(),
    )
    .await
    .unwrap();
    contract::exercise(&storage).await;
    service.shutdown().await.unwrap();
    let root = tempfile::tempdir().unwrap();
    let (service, storage) = open(&root.path().join("store.db")).await;
    contract::exercise(&storage).await;
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn committed_text_and_ids_survive_restart() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("private/store.db");
    let (service, storage) = open(&path).await;
    let text = " 中文 🦀\r\n ";
    let first = storage
        .history()
        .record(contract::record(text, 10, HistorySource::Local))
        .await
        .unwrap();
    storage
        .settings()
        .apply(change("config", "{\"name\":\"我的电脑\"}"))
        .await
        .unwrap();
    service.shutdown().await.unwrap();
    assert_eq!(
        storage.history().get(first.id).await.unwrap_err().kind(),
        ErrorKind::Stopped
    );
    let (service, storage) = open(&path).await;
    assert_eq!(
        storage
            .history()
            .get(first.id)
            .await
            .unwrap()
            .unwrap()
            .entry
            .text,
        text
    );
    let second = storage
        .history()
        .record(contract::record(
            text,
            20,
            HistorySource::Remote("peer".into()),
        ))
        .await
        .unwrap();
    assert_eq!(first.id, second.id);
    assert_eq!(
        storage.settings().get("config").await.unwrap().as_deref(),
        Some("{\"name\":\"我的电脑\"}")
    );
    service.shutdown().await.unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[tokio::test]
async fn independent_services_deduplicate_atomically_across_connections() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("store.db");
    let (a_owner, a) = open(&path).await;
    let (b_owner, b) = open(&path).await;
    let mut calls = Vec::new();
    for index in 0..32 {
        let storage = if index % 2 == 0 { a.clone() } else { b.clone() };
        calls.push(tokio::spawn(async move {
            storage
                .history()
                .record(contract::record(
                    "same exact text",
                    index,
                    HistorySource::Local,
                ))
                .await
                .unwrap()
        }));
    }
    let mut ids = std::collections::HashSet::new();
    let mut inserts = 0;
    for call in calls {
        let result = call.await.unwrap();
        ids.insert(result.id);
        inserts += usize::from(result.inserted);
    }
    assert_eq!(ids.len(), 1);
    assert_eq!(inserts, 1);
    assert_eq!(
        a.history()
            .query(HistoryQuery::default())
            .await
            .unwrap()
            .entries
            .len(),
        1
    );
    a_owner.shutdown().await.unwrap();
    b_owner.shutdown().await.unwrap();
}

#[tokio::test]
async fn settings_and_history_failures_roll_back_the_entire_operation() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("store.db");
    let (service, storage) = open(&path).await;
    storage
        .settings()
        .apply(change("keep", "original"))
        .await
        .unwrap();
    let old = storage
        .history()
        .record(contract::record("old", 1, HistorySource::Local))
        .await
        .unwrap();
    let raw = Connection::open(&path).unwrap();
    raw.execute_batch("CREATE TRIGGER fail_setting BEFORE INSERT ON settings WHEN NEW.key='fail' BEGIN SELECT RAISE(ABORT,'injected'); END;
        CREATE TRIGGER fail_prune BEFORE DELETE ON history BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    let result = storage
        .settings()
        .apply(SettingsChanges {
            put: vec![
                Setting {
                    key: "keep".into(),
                    value: "must roll back".into(),
                },
                Setting {
                    key: "fail".into(),
                    value: "x".into(),
                },
            ],
            delete: vec![],
        })
        .await;
    assert_eq!(result.unwrap_err().kind(), ErrorKind::Conflict);
    assert_eq!(
        storage.settings().get("keep").await.unwrap().as_deref(),
        Some("original")
    );
    assert_eq!(storage.settings().get("fail").await.unwrap(), None);
    let mut input = contract::record("new", 2, HistorySource::Local);
    input.retention.max_entries = 1;
    assert_eq!(
        storage.history().record(input).await.unwrap_err().kind(),
        ErrorKind::Conflict
    );
    let rows = storage
        .history()
        .query(HistoryQuery::default())
        .await
        .unwrap();
    assert_eq!(rows.entries.len(), 1);
    assert_eq!(rows.entries[0].id, old.id);
    raw.execute_batch("DROP TRIGGER fail_setting; DROP TRIGGER fail_prune;")
        .unwrap();
    storage
        .settings()
        .apply(change("keep", "recovered"))
        .await
        .unwrap();
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn lock_contention_is_busy_and_recovers_without_reopening() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("store.db");
    let mut options = SqliteOptions::file(&path);
    options.busy_timeout = Duration::from_millis(10);
    let (service, storage) =
        StorageService::start(BackendConfig::Sqlite(options), Options::default())
            .await
            .unwrap();
    let raw = Connection::open(&path).unwrap();
    raw.execute_batch("BEGIN IMMEDIATE;").unwrap();
    let error = storage
        .settings()
        .apply(change("key", "value"))
        .await
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Busy);
    assert!(std::error::Error::source(&error).is_some());
    raw.execute_batch("ROLLBACK;").unwrap();
    storage
        .settings()
        .apply(change("key", "value"))
        .await
        .unwrap();
    service.shutdown().await.unwrap();
}

fn legacy(path: &Path, setting: &[u8]) {
    let raw = Connection::open(path).unwrap();
    raw.execute_batch("CREATE TABLE history(id INTEGER PRIMARY KEY AUTOINCREMENT, text TEXT NOT NULL UNIQUE, source TEXT NOT NULL, copied_at_ms INTEGER NOT NULL);
        CREATE INDEX history_recent ON history(copied_at_ms DESC,id DESC);
        CREATE TABLE settings(key TEXT PRIMARY KEY,value BLOB NOT NULL);
        INSERT INTO history(id,text,source,copied_at_ms) VALUES(7,'旧记录 🦀','local',10),(15,'远端记录','peer',20),(99,'deleted','local',30);
        DELETE FROM history WHERE id=99;
        PRAGMA user_version=1;").unwrap();
    raw.execute(
        "INSERT INTO settings(key,value) VALUES('configuration_v2',?1)",
        [setting],
    )
    .unwrap();
}

#[tokio::test]
async fn legacy_schema_migrates_text_settings_sources_and_id_watermark() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("store.db");
    legacy(&path, b"{\"version\":2}");
    let (service, storage) = open(&path).await;
    assert_eq!(
        storage
            .history()
            .get(HistoryId::try_from(7).unwrap())
            .await
            .unwrap()
            .unwrap()
            .entry
            .source,
        HistorySource::Local
    );
    assert_eq!(
        storage
            .history()
            .get(HistoryId::try_from(15).unwrap())
            .await
            .unwrap()
            .unwrap()
            .entry
            .source,
        HistorySource::Remote("peer".into())
    );
    assert_eq!(
        storage
            .settings()
            .get("configuration_v2")
            .await
            .unwrap()
            .as_deref(),
        Some("{\"version\":2}")
    );
    let same = storage
        .history()
        .record(contract::record("旧记录 🦀", 30, HistorySource::Local))
        .await
        .unwrap();
    assert_eq!(same.id.value(), 7);
    let new = storage
        .history()
        .record(contract::record("new", 40, HistorySource::Local))
        .await
        .unwrap();
    assert!(new.id.value() > 99);
    service.shutdown().await.unwrap();
    let raw = Connection::open(&path).unwrap();
    assert_eq!(
        raw.pragma_query_value::<i64, _>(None, "user_version", |r| r.get(0))
            .unwrap(),
        2
    );
    assert_eq!(
        raw.query_row::<String, _, _>("SELECT typeof(value) FROM settings", [], |r| r.get(0))
            .unwrap(),
        "text"
    );
}

#[tokio::test]
async fn invalid_legacy_document_rolls_back_schema_and_data() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("store.db");
    legacy(&path, &[0xff, 0xfe]);
    let result = StorageService::start(
        BackendConfig::Sqlite(SqliteOptions::file(&path)),
        Options::default(),
    )
    .await;
    assert_eq!(result.err().unwrap().kind(), ErrorKind::Corrupt);
    let raw = Connection::open(&path).unwrap();
    assert_eq!(
        raw.pragma_query_value::<i64, _>(None, "user_version", |r| r.get(0))
            .unwrap(),
        1
    );
    assert_eq!(
        raw.query_row::<Vec<u8>, _, _>("SELECT value FROM settings", [], |r| r.get(0))
            .unwrap(),
        vec![0xff, 0xfe]
    );
    assert_eq!(
        raw.query_row::<String, _, _>("SELECT source FROM history WHERE id=7", [], |r| r.get(0))
            .unwrap(),
        "local"
    );
    let renamed: i64 = raw
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE name LIKE 'legacy_%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(renamed, 0);
}

#[tokio::test]
async fn future_schema_and_invalid_options_fail_without_returning_a_handle() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("store.db");
    let raw = Connection::open(&path).unwrap();
    raw.execute_batch("PRAGMA user_version=999;").unwrap();
    let result = StorageService::start(
        BackendConfig::Sqlite(SqliteOptions::file(&path)),
        Options::default(),
    )
    .await;
    assert_eq!(result.err().unwrap().kind(), ErrorKind::SchemaTooNew);
    assert_eq!(
        raw.pragma_query_value::<i64, _>(None, "user_version", |r| r.get(0))
            .unwrap(),
        999
    );
    let unused = root.path().join("must-not-create.db");
    let result = StorageService::start(
        BackendConfig::Sqlite(SqliteOptions::file(&unused)),
        Options { queue_capacity: 0 },
    )
    .await;
    assert_eq!(result.err().unwrap().kind(), ErrorKind::InvalidInput);
    assert!(!unused.exists());
}

#[tokio::test]
async fn settings_reads_do_not_mix_versions_from_concurrent_transactions() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("store.db");
    let (a_owner, a) = open(&path).await;
    let (b_owner, b) = open(&path).await;
    let writer = tokio::spawn(async move {
        for i in 0..100 {
            b.settings()
                .apply(SettingsChanges {
                    put: vec![
                        Setting {
                            key: "a".into(),
                            value: i.to_string(),
                        },
                        Setting {
                            key: "b".into(),
                            value: i.to_string(),
                        },
                    ],
                    delete: vec![],
                })
                .await
                .unwrap();
        }
    });
    for _ in 0..100 {
        let values = a
            .settings()
            .get_many(vec!["a".into(), "b".into()])
            .await
            .unwrap();
        assert_eq!(values[0], values[1]);
    }
    writer.await.unwrap();
    a_owner.shutdown().await.unwrap();
    b_owner.shutdown().await.unwrap();
}

#[tokio::test]
async fn reopen_after_a_real_write_failure_preserves_previous_rows() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("store.db");
    let (service, storage) = open(&path).await;
    storage
        .settings()
        .apply(change("value", "original"))
        .await
        .unwrap();
    let raw = Connection::open(&path).unwrap();
    raw.execute_batch("CREATE TRIGGER fail_update BEFORE UPDATE ON settings BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert_eq!(
        storage
            .settings()
            .apply(change("value", "replacement"))
            .await
            .unwrap_err()
            .kind(),
        ErrorKind::Conflict
    );
    service.shutdown().await.unwrap();
    raw.execute_batch("DROP TRIGGER fail_update;").unwrap();
    drop(raw);
    let (service, storage) = open(&path).await;
    assert_eq!(
        storage.settings().get("value").await.unwrap().as_deref(),
        Some("original")
    );
    service.shutdown().await.unwrap();
}
