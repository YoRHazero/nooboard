use super::*;

pub async fn exercise(storage: &Storage) {
    let history = storage.history();
    let original = " 中文 🦀\r\n code\n ";
    let first = history
        .record(record(original, 1, HistorySource::Local))
        .await
        .unwrap();
    let again = history
        .record(record(original, 2, HistorySource::Remote("local".into())))
        .await
        .unwrap();
    assert_eq!(first.id, again.id);
    assert!(first.inserted && first.retained);
    assert!(!again.inserted && again.retained);
    let saved = history.get(first.id).await.unwrap().unwrap();
    assert_eq!(saved.entry.text, original);
    assert_eq!(saved.entry.source, HistorySource::Remote("local".into()));
    assert_eq!(saved.entry.copied_at_ms, 2);
    assert!(!format!("{saved:?}").contains(original));

    let large = "文".repeat(MAX_TEXT_BYTES / 3);
    let large_id = history
        .record(record(&large, 3, HistorySource::Local))
        .await
        .unwrap()
        .id;
    assert_eq!(
        history.get(large_id).await.unwrap().unwrap().entry.text,
        large
    );
    let empty = history
        .record(record("", 4, HistorySource::Local))
        .await
        .unwrap()
        .id;
    assert_eq!(history.get(empty).await.unwrap().unwrap().entry.text, "");
    assert_eq!(history.clear().await.unwrap(), 3);
    assert!(history.get(empty).await.unwrap().is_none());

    for index in 0..80 {
        let source = if index % 2 == 0 {
            HistorySource::Local
        } else {
            HistorySource::Remote("device-a".into())
        };
        history
            .record(record(&format!("copy_%_{index}"), index, source))
            .await
            .unwrap();
    }
    let first_page = history
        .query(HistoryQuery {
            contains: "_%_".into(),
            source: SourceFilter::Local,
            limit: 32,
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(first_page.entries.len(), 32);
    assert_eq!(first_page.next_offset, Some(32));
    assert_eq!(first_page.entries[0].entry.text, "copy_%_78");
    let last = history
        .query(HistoryQuery {
            contains: "_%_".into(),
            source: SourceFilter::Local,
            limit: 32,
            offset: 32,
        })
        .await
        .unwrap();
    assert_eq!(last.entries.len(), 8);
    assert_eq!(last.next_offset, None);
    assert_eq!(last.entries[0].entry.text, "copy_%_14");
    let remote = history
        .query(HistoryQuery {
            source: SourceFilter::Device("device-a".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(remote.entries.len(), 40);
    assert!(
        history
            .query(HistoryQuery {
                contains: "COPY".into(),
                ..Default::default()
            })
            .await
            .unwrap()
            .entries
            .is_empty()
    );
    assert!(
        history
            .query(HistoryQuery {
                source: SourceFilter::Device("missing".into()),
                ..Default::default()
            })
            .await
            .unwrap()
            .entries
            .is_empty()
    );

    let invalid = [
        record("nul\0text", 90, HistorySource::Local),
        record("valid", -1, HistorySource::Local),
        record("valid", 90, HistorySource::Remote("".into())),
    ];
    for input in invalid {
        assert_eq!(
            history.record(input).await.unwrap_err().kind(),
            ErrorKind::InvalidInput
        );
    }
    assert_eq!(
        history
            .query(HistoryQuery {
                limit: 0,
                ..Default::default()
            })
            .await
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidInput
    );
    assert_eq!(
        history
            .query(HistoryQuery {
                offset: u64::MAX,
                ..Default::default()
            })
            .await
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidInput
    );

    let mut backdated = record("backdated", 1, HistorySource::Local);
    backdated.retention.max_entries = 2;
    let outcome = history.record(backdated).await.unwrap();
    assert!(outcome.inserted && !outcome.retained);
    assert_eq!(outcome.pruned, 79);
    assert_eq!(
        history
            .prune(Retention {
                max_entries: 10,
                oldest_ms: 79
            })
            .await
            .unwrap(),
        1
    );
    let remaining = history.query(HistoryQuery::default()).await.unwrap();
    assert_eq!(remaining.entries.len(), 1);
    assert_eq!(remaining.entries[0].entry.text, "copy_%_79");
    assert!(history.delete(remaining.entries[0].id).await.unwrap());
    assert!(!history.delete(remaining.entries[0].id).await.unwrap());
    history
        .record(record("zero retention", 100, HistorySource::Local))
        .await
        .unwrap();
    assert_eq!(
        history
            .prune(Retention {
                max_entries: 0,
                oldest_ms: 0
            })
            .await
            .unwrap(),
        1
    );
    assert_eq!(history.clear().await.unwrap(), 0);
}
