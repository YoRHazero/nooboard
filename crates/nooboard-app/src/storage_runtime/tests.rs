use std::path::Path;

use tempfile::TempDir;
use uuid::Uuid;

use super::StorageRuntime;
use crate::AppError;

fn make_config(
    db_root: &Path,
    lifecycle: nooboard_storage::LifecycleConfig,
) -> nooboard_storage::AppConfig {
    nooboard_storage::AppConfig {
        storage: nooboard_storage::StorageConfig {
            db_root: db_root.to_path_buf(),
            retain_old_versions: 0,
            lifecycle,
        },
    }
}

fn valid_lifecycle() -> nooboard_storage::LifecycleConfig {
    nooboard_storage::LifecycleConfig {
        history_window_days: 7,
        dedup_window_days: 14,
        gc_every_inserts: 200,
        gc_batch_size: 500,
    }
}

#[test]
fn startup_fails_for_invalid_storage_config() {
    let dir = TempDir::new().expect("temp dir");
    let lifecycle = nooboard_storage::LifecycleConfig {
        gc_every_inserts: 0,
        ..valid_lifecycle()
    };
    let config = make_config(dir.path().join("db").as_path(), lifecycle);

    let result = StorageRuntime::new(config);
    assert!(matches!(
        result,
        Err(AppError::Storage(
            nooboard_storage::StorageError::InvalidConfig(_)
        ))
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn append_and_list_history_roundtrip() {
    let dir = TempDir::new().expect("temp dir");
    let config = make_config(dir.path().join("db-a").as_path(), valid_lifecycle());
    let runtime = StorageRuntime::new(config).expect("runtime");

    let event_id = Uuid::now_v7();
    runtime
        .append_text("alpha", Some(event_id), Some("device-a"), 1000, 1000)
        .await
        .expect("append");

    let records = runtime.list_history(10, None).await.expect("list");
    assert_eq!(records.len(), 1);
    assert_eq!(Uuid::from_bytes(records[0].event_id), event_id);
    assert_eq!(records[0].content, "alpha");
    assert_eq!(records[0].origin_device_id, "device-a");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reconfigure_switches_active_database() {
    let dir = TempDir::new().expect("temp dir");
    let config_a = make_config(dir.path().join("db-a").as_path(), valid_lifecycle());
    let config_b = make_config(dir.path().join("db-b").as_path(), valid_lifecycle());
    let runtime = StorageRuntime::new(config_a.clone()).expect("runtime");

    runtime
        .append_text("from-a", Some(Uuid::now_v7()), Some("device-a"), 1000, 1000)
        .await
        .expect("append a");
    assert_eq!(
        runtime
            .list_history(10, None)
            .await
            .expect("list a before")
            .len(),
        1
    );

    runtime
        .reconfigure(config_b.clone())
        .await
        .expect("reconfigure to b");
    let after_switch = runtime.list_history(10, None).await.expect("list b");
    assert!(after_switch.is_empty());

    runtime
        .append_text("from-b", Some(Uuid::now_v7()), Some("device-b"), 2000, 2000)
        .await
        .expect("append b");
    let b_records = runtime.list_history(10, None).await.expect("list b after");
    assert_eq!(b_records.len(), 1);
    assert_eq!(b_records[0].content, "from-b");

    runtime
        .reconfigure(config_a)
        .await
        .expect("reconfigure back to a");
    let a_records = runtime.list_history(10, None).await.expect("list a after");
    assert_eq!(a_records.len(), 1);
    assert_eq!(a_records[0].content, "from-a");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reconfigure_with_same_config_is_safe() {
    let dir = TempDir::new().expect("temp dir");
    let config = make_config(dir.path().join("db-a").as_path(), valid_lifecycle());
    let runtime = StorageRuntime::new(config.clone()).expect("runtime");

    runtime
        .append_text("before", Some(Uuid::now_v7()), Some("device-a"), 1000, 1000)
        .await
        .expect("append before");
    runtime.reconfigure(config).await.expect("reconfigure same");

    let records = runtime.list_history(10, None).await.expect("list");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].content, "before");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn outbox_roundtrip_and_retry_flow() {
    let dir = TempDir::new().expect("temp dir");
    let config = make_config(dir.path().join("db-a").as_path(), valid_lifecycle());
    let runtime = StorageRuntime::new(config).expect("runtime");

    let event_id = Uuid::now_v7();
    runtime
        .append_text_with_outbox(
            "payload",
            event_id,
            Some("device-a"),
            100,
            100,
            Some(vec!["peer-b".to_string(), "peer-a".to_string()]),
            100,
        )
        .await
        .expect("append with outbox");

    let due = runtime.list_due_outbox(100, 10).await.expect("list due");
    assert_eq!(due.len(), 2);
    assert!(
        due.iter()
            .all(|message| Uuid::from_bytes(message.event_id) == event_id)
    );
    let mut due_targets = due
        .iter()
        .filter_map(|message| message.targets.as_ref())
        .filter_map(|targets| targets.first())
        .cloned()
        .collect::<Vec<_>>();
    due_targets.sort();
    assert_eq!(
        due_targets,
        vec!["peer-a".to_string(), "peer-b".to_string()]
    );

    assert!(
        runtime
            .try_lease_outbox_message(due[0].id, 200, 100)
            .await
            .expect("lease")
    );
    assert!(
        runtime
            .mark_outbox_retry(due[0].id, 400, "temporary failure".to_string(), 110)
            .await
            .expect("retry")
    );

    let due_before_retry = runtime
        .list_due_outbox(399, 10)
        .await
        .expect("before retry due");
    assert_eq!(due_before_retry.len(), 1);
    assert_eq!(due_before_retry[0].attempt_count, 0);
    let due_again = runtime.list_due_outbox(400, 10).await.expect("due again");
    assert_eq!(due_again.len(), 2);
    let retry_row = due_again
        .iter()
        .find(|message| message.attempt_count == 1)
        .expect("must have retried row");
    assert_eq!(retry_row.next_attempt_at_ms, 400);

    for message in due_again {
        assert!(
            runtime
                .mark_outbox_sent(message.id, 500)
                .await
                .expect("ack")
        );
    }
    assert!(
        runtime
            .list_due_outbox(1_000, 10)
            .await
            .expect("empty")
            .is_empty()
    );
}
