use nooboard_storage::{Database, HistoryQuery};
#[test]
fn history_survives_restart_and_preserves_exact_unicode() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("history.db");
    let text = " 中文 🦀\r\n code\n ";
    let id = {
        let mut db = Database::open(&path).unwrap();
        db.set_setting("peer", b"configuration").unwrap();
        db.record_text(text, "local", 100, 3, 0).unwrap()
    };
    let mut db = Database::open(&path).unwrap();
    assert_eq!(db.history_entry(id).unwrap().unwrap().text, text);
    assert_eq!(db.setting("peer").unwrap().unwrap(), b"configuration");
    assert_eq!(db.record_text(text, "remote", 200, 3, 0).unwrap(), id);
    let entries = db.history(HistoryQuery::default()).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].source, "remote");
    assert!(db.delete_history(id).unwrap());
    assert!(db.history(HistoryQuery::default()).unwrap().is_empty());
}
#[test]
fn retention_search_and_clear_are_literal_and_deterministic() {
    let mut db = Database::in_memory().unwrap();
    for (text, time) in [("old", 1), ("100%_literal", 5), ("last", 6)] {
        db.record_text(text, "local", time, 2, 0).unwrap();
    }
    let rows = db
        .history(HistoryQuery {
            contains: "%_",
            ..Default::default()
        })
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].text, "100%_literal");
    assert_eq!(db.history(HistoryQuery::default()).unwrap()[0].text, "last");
    db.prune_history(2, 6).unwrap();
    assert_eq!(db.history(HistoryQuery::default()).unwrap().len(), 1);
    db.clear_history().unwrap();
    assert!(db.history(HistoryQuery::default()).unwrap().is_empty());
}
