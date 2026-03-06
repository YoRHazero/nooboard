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
        .append_text(
            "alpha",
            Some(event_id),
            Some("noob-a"),
            Some("device-a"),
            1000,
            1000,
        )
        .await
        .expect("append");

    let records = runtime.list_history(10, None).await.expect("list");
    assert_eq!(records.len(), 1);
    assert_eq!(Uuid::from_bytes(records[0].event_id), event_id);
    assert_eq!(records[0].content, "alpha");
    assert_eq!(records[0].origin_noob_id, "noob-a");
    assert_eq!(records[0].origin_device_id, "device-a");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reconfigure_switches_active_database() {
    let dir = TempDir::new().expect("temp dir");
    let config_a = make_config(dir.path().join("db-a").as_path(), valid_lifecycle());
    let config_b = make_config(dir.path().join("db-b").as_path(), valid_lifecycle());
    let runtime = StorageRuntime::new(config_a.clone()).expect("runtime");

    runtime
        .append_text(
            "from-a",
            Some(Uuid::now_v7()),
            Some("noob-a"),
            Some("device-a"),
            1000,
            1000,
        )
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
        .append_text(
            "from-b",
            Some(Uuid::now_v7()),
            Some("noob-b"),
            Some("device-b"),
            2000,
            2000,
        )
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
        .append_text(
            "before",
            Some(Uuid::now_v7()),
            Some("noob-a"),
            Some("device-a"),
            1000,
            1000,
        )
        .await
        .expect("append before");
    runtime.reconfigure(config).await.expect("reconfigure same");

    let records = runtime.list_history(10, None).await.expect("list");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].content, "before");
}
