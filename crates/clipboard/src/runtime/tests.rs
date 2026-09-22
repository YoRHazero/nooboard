use super::*;
use crate::{Clipboard, ClipboardService, Payload};
use std::sync::{Mutex, atomic::AtomicUsize};

struct Control {
    content: Mutex<ReadState>,
    revision: std::sync::atomic::AtomicU64,
    blocked: AtomicBool,
    failure: Mutex<Option<Error>>,
    fatal: Mutex<Option<Error>>,
    polls: AtomicUsize,
    started: AtomicUsize,
    cancelled: AtomicUsize,
    dropped: AtomicBool,
}
impl Control {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            content: Mutex::new(ReadState::Ready(Payload::Text("already copied".into()))),
            revision: std::sync::atomic::AtomicU64::new(500),
            blocked: AtomicBool::new(false),
            failure: Mutex::new(None),
            fatal: Mutex::new(None),
            polls: AtomicUsize::new(0),
            started: AtomicUsize::new(0),
            cancelled: AtomicUsize::new(0),
            dropped: AtomicBool::new(false),
        })
    }
    fn external(&self, revision: u64, text: &str) {
        *self.content.lock().unwrap() = ReadState::Ready(Payload::Text(text.into()));
        self.revision.store(revision, Ordering::SeqCst);
    }
}
struct Fake {
    control: Arc<Control>,
    operation: Option<Operation>,
}
impl Backend for Fake {
    fn revision(&self) -> u64 {
        self.control.revision.load(Ordering::SeqCst)
    }
    fn begin(&mut self, operation: Operation) -> Result<()> {
        assert!(self.operation.is_none());
        self.operation = Some(operation);
        self.control.started.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn poll(&mut self) -> Result<Progress> {
        self.control.polls.fetch_add(1, Ordering::SeqCst);
        if let Some(error) = self.control.fatal.lock().unwrap().take() {
            return Err(error);
        }
        if self.operation.is_none() {
            return Ok(Progress::Idle);
        }
        if matches!(self.operation, Some(Operation::Read))
            && self.control.blocked.load(Ordering::SeqCst)
        {
            return Ok(Progress::Pending);
        }
        let operation = self.operation.take().unwrap();
        if let Some(error) = self.control.failure.lock().unwrap().clone() {
            return Ok(Progress::Complete(Err(error)));
        }
        let written = matches!(operation, Operation::Write(_));
        if let Operation::Write(prepared) = operation {
            *self.control.content.lock().unwrap() = ReadState::Ready(prepared.payload);
            self.control.revision.fetch_add(1, Ordering::SeqCst);
        }
        Ok(Progress::Complete(Ok(Observation {
            revision: self.revision(),
            content: self.control.content.lock().unwrap().clone(),
            // Simulates macOS/Windows reads: the common runtime must attribute our own writes.
            origin: if written {
                Origin::Application
            } else {
                Origin::External
            },
        })))
    }
    fn cancel(&mut self) {
        if self.operation.take().is_some() {
            self.control.cancelled.fetch_add(1, Ordering::SeqCst);
        }
    }
    fn wait(&mut self, wake: &Wake, timeout: Duration) -> Result<()> {
        wake.wait(timeout.min(Duration::from_millis(5)));
        Ok(())
    }
}
impl Drop for Fake {
    fn drop(&mut self) {
        self.control.dropped.store(true, Ordering::SeqCst);
    }
}
async fn start(control: &Arc<Control>) -> (ClipboardService, Clipboard) {
    let control = control.clone();
    ClipboardService::start_with(
        Options {
            poll_interval: Duration::from_millis(5),
            retry_interval: Duration::from_millis(5),
            operation_timeout: Duration::from_millis(100),
            queue_capacity: 1,
            ..Options::default()
        },
        move |_| {
            Ok(Box::new(Fake {
                control,
                operation: None,
            }))
        },
    )
    .await
    .unwrap()
}
async fn until(mut condition: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn startup_is_unobserved_and_native_tokens_never_escape() {
    let control = Control::new();
    control.blocked.store(true, Ordering::SeqCst);
    let (service, clipboard) = start(&control).await;
    assert!(clipboard.subscribe().borrow().is_none());
    control.blocked.store(false, Ordering::SeqCst);
    let first = clipboard.read().await.unwrap();
    assert_eq!(first.revision, 1);
    assert_eq!(first.origin, Origin::External);
    assert_eq!(
        first.content,
        ReadState::Ready(Payload::Text("already copied".into()))
    );
    let written = clipboard
        .write(Payload::Text("local".into()))
        .await
        .unwrap();
    let read = clipboard.read().await.unwrap();
    assert_eq!(read, written);
    assert_eq!(read.origin, Origin::Application);
    control.external(0, "local"); // Native counter wrap, including equal content.
    let external = clipboard.read().await.unwrap();
    assert!(external.revision > read.revision);
    assert_eq!(external.origin, Origin::External);
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn transient_error_preserves_snapshot_and_recovery_updates_health() {
    let control = Control::new();
    let (service, clipboard) = start(&control).await;
    let original = clipboard.read().await.unwrap();
    let snapshots = clipboard.subscribe();
    let status = clipboard.subscribe_status();
    *control.failure.lock().unwrap() = Some(Error::Unavailable);
    assert_eq!(clipboard.read().await.unwrap_err(), Error::Timeout);
    assert_eq!(*snapshots.borrow(), Some(original));
    assert!(matches!(*status.borrow(), ServiceStatus::Unavailable(_)));
    *control.failure.lock().unwrap() = None;
    // Recovery is detected even if the native token has not changed, without a
    // new application request to trigger the next observation.
    until(|| *status.borrow() == ServiceStatus::Ready).await;
    assert_eq!(*status.borrow(), ServiceStatus::Ready);
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn cancelling_a_read_releases_native_state_and_allows_writes() {
    let control = Control::new();
    let (service, clipboard) = start(&control).await;
    clipboard.read().await.unwrap();
    control.blocked.store(true, Ordering::SeqCst);
    let started = control.started.load(Ordering::SeqCst);
    let copy = clipboard.clone();
    let read = tokio::spawn(async move { copy.read().await });
    until(|| control.started.load(Ordering::SeqCst) > started).await;
    read.abort();
    let _ = read.await;
    until(|| control.cancelled.load(Ordering::SeqCst) > 0).await;
    clipboard
        .write(Payload::Text("after cancellation".into()))
        .await
        .unwrap();
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn stop_bypasses_a_full_queue_and_releases_native_resources() {
    let control = Control::new();
    let (service, clipboard) = start(&control).await;
    clipboard.read().await.unwrap();
    control.blocked.store(true, Ordering::SeqCst);
    let started = control.started.load(Ordering::SeqCst);
    let copy = clipboard.clone();
    let first = tokio::spawn(async move { copy.read().await });
    until(|| control.started.load(Ordering::SeqCst) > started).await;
    let mut queued = Vec::new();
    for _ in 0..8 {
        let copy = clipboard.clone();
        queued.push(tokio::spawn(async move { copy.read().await }));
    }
    tokio::task::yield_now().await;
    tokio::time::timeout(Duration::from_secs(1), service.shutdown())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.await.unwrap().unwrap_err(), Error::Stopped);
    for task in queued {
        assert_eq!(task.await.unwrap().unwrap_err(), Error::Stopped);
    }
    assert!(control.dropped.load(Ordering::SeqCst));
    assert_eq!(clipboard.read().await.unwrap_err(), Error::Stopped);
}

#[tokio::test]
async fn fatal_backend_error_is_retained_and_owner_drop_stops_a_live_handle() {
    let control = Control::new();
    let (service, clipboard) = start(&control).await;
    clipboard.read().await.unwrap();
    let error = Error::backend("test connection", "disconnected");
    *control.fatal.lock().unwrap() = Some(error.clone());
    let status = clipboard.subscribe_status();
    until(|| matches!(*status.borrow(), ServiceStatus::Stopped { .. })).await;
    assert_eq!(clipboard.read().await.unwrap_err(), error);
    assert_eq!(service.shutdown().await.unwrap_err(), error);
    let control = Control::new();
    let (service, clipboard) = start(&control).await;
    drop(service);
    until(|| control.dropped.load(Ordering::SeqCst)).await;
    assert_eq!(clipboard.read().await.unwrap_err(), Error::Stopped);
}

#[tokio::test]
async fn idle_backend_is_pumped_and_invalid_writes_leave_state_unchanged() {
    let control = Control::new();
    let (service, clipboard) = start(&control).await;
    let original = clipboard.read().await.unwrap();
    let before = control.polls.load(Ordering::SeqCst);
    until(|| control.polls.load(Ordering::SeqCst) >= before + 3).await;
    assert_eq!(
        clipboard
            .write(Payload::Text("invalid\0text".into()))
            .await
            .unwrap_err(),
        Error::InvalidInput
    );
    assert_eq!(clipboard.read().await.unwrap(), original);
    let invalid_image = crate::ImageData::new(crate::ImageEncoding::Png, vec![1, 2, 3]).unwrap();
    assert_eq!(
        clipboard
            .write(Payload::Image(invalid_image))
            .await
            .unwrap_err(),
        Error::InvalidData
    );
    assert_eq!(*clipboard.subscribe_status().borrow(), ServiceStatus::Ready);
    service.shutdown().await.unwrap();
}
