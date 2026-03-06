use std::collections::{HashMap, VecDeque, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use nooboard_platform::{
    ClipboardBackend, ClipboardEvent, ClipboardEventSender, DEFAULT_WATCH_INTERVAL, NooboardError,
};
use tokio::sync::{broadcast, mpsc};

use crate::service::EventId;
use crate::{AppError, AppResult};

const LOCAL_CLIPBOARD_CHANNEL_CAPACITY: usize = 256;
const SUPPRESSION_TTL: Duration = Duration::from_secs(3);

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
    pub event_id: EventId,
    pub text: String,
    pub observed_at_ms: i64,
}

impl LocalClipboardObserved {
    fn from_platform_event(value: ClipboardEvent) -> Self {
        let observed_at_ms = i64::try_from(value.timestamp_millis()).unwrap_or(i64::MAX);
        Self {
            event_id: EventId::new(),
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

#[derive(Debug, Clone)]
struct SuppressionEntry {
    event_id: EventId,
    expires_at: Instant,
}

#[derive(Debug)]
struct SuppressionState {
    by_fingerprint: HashMap<u64, VecDeque<SuppressionEntry>>,
    ttl: Duration,
}

impl SuppressionState {
    fn new(ttl: Duration) -> Self {
        Self {
            by_fingerprint: HashMap::new(),
            ttl,
        }
    }

    fn register(&mut self, event_id: EventId, text: &str) {
        let fingerprint = fingerprint_text(text);
        let entry = SuppressionEntry {
            event_id,
            expires_at: Instant::now() + self.ttl,
        };
        self.by_fingerprint
            .entry(fingerprint)
            .or_default()
            .push_back(entry);
        self.prune_expired_for(fingerprint);
    }

    fn should_suppress(&mut self, text: &str) -> bool {
        let fingerprint = fingerprint_text(text);
        self.prune_expired_for(fingerprint);
        let Some(entries) = self.by_fingerprint.get_mut(&fingerprint) else {
            return false;
        };

        let should_drop = entries.pop_front().map(|entry| entry.event_id).is_some();
        if entries.is_empty() {
            self.by_fingerprint.remove(&fingerprint);
        }
        should_drop
    }

    fn prune_expired_for(&mut self, fingerprint: u64) {
        let now = Instant::now();
        if let Some(entries) = self.by_fingerprint.get_mut(&fingerprint) {
            while entries.front().is_some_and(|entry| entry.expires_at <= now) {
                let _ = entries.pop_front();
            }
            if entries.is_empty() {
                self.by_fingerprint.remove(&fingerprint);
            }
        }
    }
}

#[derive(Clone)]
pub struct ClipboardRuntime {
    backend: Arc<dyn ClipboardPort>,
    watch: Arc<Mutex<WatchState>>,
    suppression: Arc<Mutex<SuppressionState>>,
}

impl ClipboardRuntime {
    pub fn new(backend: Arc<dyn ClipboardPort>) -> Self {
        Self {
            backend,
            watch: Arc::new(Mutex::new(WatchState::new(DEFAULT_WATCH_INTERVAL))),
            suppression: Arc::new(Mutex::new(SuppressionState::new(SUPPRESSION_TTL))),
        }
    }

    pub fn read_text(&self) -> AppResult<Option<String>> {
        self.backend.read_text()
    }

    pub fn write_text_with_event(&self, event_id: EventId, text: &str) -> AppResult<()> {
        self.suppression
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .register(event_id, text);
        self.backend.write_text(text)
    }

    pub fn start_watch(&self) -> AppResult<()> {
        self.ensure_watch_started()
    }

    pub fn subscribe_local_changes(&self) -> AppResult<LocalClipboardSubscription> {
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
        let suppression = Arc::clone(&self.suppression);
        let forward_task = handle.spawn(async move {
            while let Some(event) = platform_rx.recv().await {
                let should_drop = suppression
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .should_suppress(event.text.as_str());
                if should_drop {
                    continue;
                }
                let _ = events_tx.send(LocalClipboardObserved::from_platform_event(event));
            }
        });

        state.started = true;
        state.shutdown = Some(shutdown);
        state.worker = Some(worker);
        state.forward_task = Some(forward_task);

        Ok(())
    }
}

fn fingerprint_text(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}
