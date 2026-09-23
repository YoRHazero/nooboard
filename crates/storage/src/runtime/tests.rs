use crate::{
    backend::contract::{Backend, BackendFuture},
    *,
};
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    task::Poll,
    time::Duration,
};
use tokio::sync::{Notify, Semaphore};

struct Control {
    started: Notify,
    gate: Semaphore,
    calls: AtomicUsize,
    closed: AtomicBool,
    fail: AtomicBool,
    value: Mutex<Option<String>>,
}
impl Control {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            started: Notify::new(),
            gate: Semaphore::new(0),
            calls: AtomicUsize::new(0),
            closed: AtomicBool::new(false),
            fail: AtomicBool::new(false),
            value: Mutex::new(None),
        })
    }
}
struct Fake(Arc<Control>);
impl Backend for Fake {
    fn record_history(&self, _: RecordHistory) -> BackendFuture<'_, RecordOutcome> {
        Box::pin(async { panic!("unexpected history request") })
    }
    fn query_history(&self, _: HistoryQuery) -> BackendFuture<'_, HistoryPage> {
        Box::pin(async { panic!("unexpected history request") })
    }
    fn get_history(&self, _: HistoryId) -> BackendFuture<'_, Option<HistoryEntry>> {
        Box::pin(async { panic!("unexpected history request") })
    }
    fn delete_history(&self, _: HistoryId) -> BackendFuture<'_, bool> {
        Box::pin(async { panic!("unexpected history request") })
    }
    fn prune_history(&self, _: Retention) -> BackendFuture<'_, u64> {
        Box::pin(async { panic!("unexpected history request") })
    }
    fn clear_history(&self) -> BackendFuture<'_, u64> {
        Box::pin(async { panic!("unexpected history request") })
    }
    fn read_settings(&self, keys: Vec<String>) -> BackendFuture<'_, Vec<Option<String>>> {
        Box::pin(async move {
            Ok(keys
                .iter()
                .map(|_| self.0.value.lock().unwrap().clone())
                .collect())
        })
    }
    fn apply_settings(&self, changes: SettingsChanges) -> BackendFuture<'_, ()> {
        Box::pin(async move {
            self.0.calls.fetch_add(1, Ordering::SeqCst);
            self.0.started.notify_one();
            self.0.gate.acquire().await.unwrap().forget();
            if self.0.fail.swap(false, Ordering::SeqCst) {
                return Err(Error::new(ErrorKind::Unavailable, "test backend"));
            }
            *self.0.value.lock().unwrap() = changes.put.first().map(|s| s.value.clone());
            Ok(())
        })
    }
    fn close(&self) -> BackendFuture<'_, ()> {
        Box::pin(async {
            self.0.closed.store(true, Ordering::SeqCst);
            Ok(())
        })
    }
}
fn start(control: &Arc<Control>) -> (StorageService, Storage) {
    StorageService::spawn(
        Box::new(Fake(control.clone())),
        Options { queue_capacity: 1 },
    )
}
fn change(value: &str) -> SettingsChanges {
    SettingsChanges {
        put: vec![Setting {
            key: "key".into(),
            value: value.into(),
        }],
        delete: vec![],
    }
}
async fn pending<T>(mut future: Pin<&mut impl Future<Output = T>>) {
    std::future::poll_fn(|cx| {
        assert!(future.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
}
async fn until(mut check: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while !check() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn shutdown_bypasses_saturation_but_finishes_the_active_commit() {
    let control = Control::new();
    let (service, storage) = start(&control);
    let settings = storage.settings();
    let mut active = Box::pin(settings.apply(change("committed")));
    pending(active.as_mut()).await;
    control.started.notified().await;
    let mut queued = Box::pin(settings.apply(change("queued")));
    pending(queued.as_mut()).await;
    let mut backpressured = Box::pin(settings.apply(change("not enqueued")));
    pending(backpressured.as_mut()).await;
    assert_eq!(control.calls.load(Ordering::SeqCst), 1);

    let mut shutdown = Box::pin(service.shutdown());
    pending(shutdown.as_mut()).await;
    assert_eq!(
        settings.get("key").await.unwrap_err().kind(),
        ErrorKind::Stopped
    );
    assert_eq!(backpressured.await.unwrap_err().kind(), ErrorKind::Stopped);
    assert!(!control.closed.load(Ordering::SeqCst));
    control.gate.add_permits(1);
    active.await.unwrap();
    assert_eq!(queued.await.unwrap_err().kind(), ErrorKind::Stopped);
    shutdown.await.unwrap();
    assert_eq!(control.calls.load(Ordering::SeqCst), 1);
    assert_eq!(control.value.lock().unwrap().as_deref(), Some("committed"));
    assert!(control.closed.load(Ordering::SeqCst));
}

#[tokio::test]
async fn cancelled_queued_write_is_skipped_and_cancelled_active_write_finishes() {
    let control = Control::new();
    let (service, storage) = start(&control);
    let settings = storage.settings();
    let mut active = Box::pin(settings.apply(change("kept")));
    pending(active.as_mut()).await;
    control.started.notified().await;
    let mut queued = Box::pin(settings.apply(change("cancelled")));
    pending(queued.as_mut()).await;
    drop(queued);
    drop(active);
    control.gate.add_permits(1);
    assert_eq!(settings.get("key").await.unwrap().as_deref(), Some("kept"));
    assert_eq!(control.calls.load(Ordering::SeqCst), 1);
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn backend_error_is_returned_without_stopping_later_requests() {
    let control = Control::new();
    control.fail.store(true, Ordering::SeqCst);
    control.gate.add_permits(2);
    let (service, storage) = start(&control);
    assert_eq!(
        storage
            .settings()
            .apply(change("failed"))
            .await
            .unwrap_err()
            .kind(),
        ErrorKind::Unavailable
    );
    storage.settings().apply(change("recovered")).await.unwrap();
    assert_eq!(
        storage.settings().get("key").await.unwrap().as_deref(),
        Some("recovered")
    );
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn dropping_owner_stops_live_handles_and_dropping_all_handles_closes_backend() {
    let control = Control::new();
    let (service, storage) = start(&control);
    drop(service);
    until(|| control.closed.load(Ordering::SeqCst)).await;
    assert_eq!(
        storage.settings().get("key").await.unwrap_err().kind(),
        ErrorKind::Stopped
    );
    let control = Control::new();
    let (service, storage) = start(&control);
    drop(storage);
    until(|| control.closed.load(Ordering::SeqCst)).await;
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn invalid_batches_never_reach_backend() {
    let control = Control::new();
    let (service, storage) = start(&control);
    let mut changes = change("private document");
    changes.delete.push("key".into());
    assert_eq!(
        storage.settings().apply(changes).await.unwrap_err().kind(),
        ErrorKind::InvalidInput
    );
    assert_eq!(control.calls.load(Ordering::SeqCst), 0);
    service.shutdown().await.unwrap();
}
