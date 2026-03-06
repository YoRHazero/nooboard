use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use nooboard_platform::{
    ClipboardBackend, ClipboardEvent, ClipboardEventSender, DEFAULT_WATCH_INTERVAL, NooboardError,
};
use tokio::sync::{broadcast, mpsc};

use crate::{AppError, AppResult};

const LOCAL_CLIPBOARD_CHANNEL_CAPACITY: usize = 256;

pub trait ClipboardPort: Send + Sync {
    fn read_text(&self) -> AppResult<Option<String>>;
    fn write_text(&self, text: &str) -> AppResult<()>;
    fn watch_changes(
        &self,
        _sender: ClipboardEventSender,
        _shutdown: Arc<AtomicBool>,
        _interval: Duration,
    ) -> AppResult<JoinHandle<()>> {
        Err(
            NooboardError::platform("watch_changes is not supported by this clipboard backend")
                .into(),
        )
    }
}

impl<T> ClipboardPort for T
where
    T: ClipboardBackend,
{
    fn read_text(&self) -> AppResult<Option<String>> {
        ClipboardBackend::read_text(self).map_err(Into::into)
    }

    fn write_text(&self, text: &str) -> AppResult<()> {
        ClipboardBackend::write_text(self, text).map_err(Into::into)
    }

    fn watch_changes(
        &self,
        sender: ClipboardEventSender,
        shutdown: Arc<AtomicBool>,
        interval: Duration,
    ) -> AppResult<JoinHandle<()>> {
        ClipboardBackend::watch_changes(self, sender, shutdown, interval).map_err(Into::into)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalClipboardObserved {
    pub event_id: uuid::Uuid,
    pub text: String,
    pub observed_at_ms: i64,
}

impl From<ClipboardEvent> for LocalClipboardObserved {
    fn from(value: ClipboardEvent) -> Self {
        let observed_at_ms = i64::try_from(value.timestamp_millis()).unwrap_or(i64::MAX);
        Self {
            event_id: uuid::Uuid::now_v7(),
            text: value.text,
            observed_at_ms,
        }
    }
}

pub struct LocalClipboardSubscription {
    receiver: broadcast::Receiver<LocalClipboardObserved>,
}

impl LocalClipboardSubscription {
    pub(crate) fn new(receiver: broadcast::Receiver<LocalClipboardObserved>) -> Self {
        Self { receiver }
    }

    pub async fn recv(&mut self) -> Result<LocalClipboardObserved, broadcast::error::RecvError> {
        self.receiver.recv().await
    }

    pub fn try_recv(&mut self) -> Result<LocalClipboardObserved, broadcast::error::TryRecvError> {
        self.receiver.try_recv()
    }
}

struct WatchState {
    events_tx: broadcast::Sender<LocalClipboardObserved>,
    started: bool,
    shutdown: Option<Arc<AtomicBool>>,
    worker: Option<JoinHandle<()>>,
    forward_task: Option<tokio::task::JoinHandle<()>>,
    interval: Duration,
}

impl WatchState {
    fn new(interval: Duration) -> Self {
        let (events_tx, _) = broadcast::channel(LOCAL_CLIPBOARD_CHANNEL_CAPACITY);
        Self {
            events_tx,
            started: false,
            shutdown: None,
            worker: None,
            forward_task: None,
            interval,
        }
    }
}

#[derive(Clone)]
pub struct ClipboardRuntime {
    backend: Arc<dyn ClipboardPort>,
    watch: Arc<Mutex<WatchState>>,
}

impl ClipboardRuntime {
    pub fn new(backend: Arc<dyn ClipboardPort>) -> Self {
        Self {
            backend,
            watch: Arc::new(Mutex::new(WatchState::new(DEFAULT_WATCH_INTERVAL))),
        }
    }

    pub fn read_text(&self) -> AppResult<Option<String>> {
        self.backend.read_text()
    }

    pub fn write_text(&self, text: &str) -> AppResult<()> {
        self.backend.write_text(text)
    }

    pub fn subscribe_local_changes(&self) -> AppResult<LocalClipboardSubscription> {
        self.ensure_watch_started()?;
        let receiver = self
            .watch
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .events_tx
            .subscribe();
        Ok(LocalClipboardSubscription::new(receiver))
    }

    pub async fn stop_watch(&self) -> AppResult<()> {
        let (shutdown, worker, forward_task) = {
            let mut state = self
                .watch
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if !state.started {
                return Ok(());
            }

            state.started = false;
            (
                state.shutdown.take(),
                state.worker.take(),
                state.forward_task.take(),
            )
        };

        if let Some(shutdown) = shutdown {
            shutdown.store(true, Ordering::Relaxed);
        }

        if let Some(worker) = worker {
            let join_result = tokio::task::spawn_blocking(move || worker.join())
                .await
                .map_err(|error| {
                    AppError::ChannelClosed(format!(
                        "failed to join clipboard watch thread: {error}"
                    ))
                })?;
            if join_result.is_err() {
                return Err(AppError::ChannelClosed(
                    "clipboard watch thread panicked while shutting down".to_string(),
                ));
            }
        }

        if let Some(forward_task) = forward_task {
            forward_task.abort();
            let _ = forward_task.await;
        }

        Ok(())
    }

    fn ensure_watch_started(&self) -> AppResult<()> {
        let mut state = self
            .watch
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.started {
            return Ok(());
        }

        let handle = tokio::runtime::Handle::try_current().map_err(|error| {
            AppError::ChannelClosed(format!("clipboard watch requires a Tokio runtime: {error}"))
        })?;

        let (platform_tx, mut platform_rx) = mpsc::channel(LOCAL_CLIPBOARD_CHANNEL_CAPACITY);
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker =
            self.backend
                .watch_changes(platform_tx, Arc::clone(&shutdown), state.interval)?;
        let events_tx = state.events_tx.clone();
        let forward_task = handle.spawn(async move {
            while let Some(event) = platform_rx.recv().await {
                let _ = events_tx.send(LocalClipboardObserved::from(event));
            }
        });

        state.started = true;
        state.shutdown = Some(shutdown);
        state.worker = Some(worker);
        state.forward_task = Some(forward_task);

        Ok(())
    }
}
