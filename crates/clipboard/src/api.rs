use crate::{
    Error, Options, Payload, Result, ServiceStatus, Snapshot,
    backend::{self, Backend, Wake},
    runtime::{
        self,
        request::{Command, Request},
    },
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};
use tokio::sync::{mpsc, oneshot, watch};

/// Shareable asynchronous access. Dropping a handle never joins a native thread.
#[derive(Clone)]
pub struct Clipboard {
    requests: mpsc::Sender<Request>,
    wake: Arc<Wake>,
    snapshots: watch::Receiver<Option<Snapshot>>,
    status: watch::Receiver<ServiceStatus>,
}
impl Clipboard {
    /// No value means the clipboard has not yet been observed. Rapid changes may coalesce.
    pub fn subscribe(&self) -> watch::Receiver<Option<Snapshot>> {
        self.snapshots.clone()
    }
    pub fn subscribe_status(&self) -> watch::Receiver<ServiceStatus> {
        self.status.clone()
    }
    pub async fn read(&self) -> Result<Snapshot> {
        self.request(Command::Read).await
    }
    /// Images are validated and normalized before native publication. Cancelling the
    /// future skips queued work; a write already submitted to the OS is not rolled back.
    pub async fn write(&self, payload: Payload) -> Result<Snapshot> {
        self.request(Command::Write(payload)).await
    }
    async fn request(&self, command: Command) -> Result<Snapshot> {
        let (reply, result) = oneshot::channel();
        self.requests
            .send(Request { command, reply })
            .await
            .map_err(|_| self.stopped_error())?;
        self.wake.notify();
        // Declared before result: cancellation drops the receiver before waking the worker.
        let _wake_on_drop = WakeOnDrop(self.wake.clone());
        let result = result;
        result.await.map_err(|_| self.stopped_error())?
    }
    fn stopped_error(&self) -> Error {
        match &*self.status.borrow() {
            ServiceStatus::Stopped { error: Some(error) } => error.clone(),
            _ => Error::Stopped,
        }
    }
}
struct WakeOnDrop(Arc<Wake>);
impl Drop for WakeOnDrop {
    fn drop(&mut self) {
        self.0.notify();
    }
}

/// Owns the native service. Use `shutdown` to await resource release. Dropping the
/// owner signals stop without blocking the caller; the worker then cleans itself up.
pub struct ClipboardService {
    stop: Arc<AtomicBool>,
    wake: Arc<Wake>,
    thread: Option<thread::JoinHandle<Result<()>>>,
}
impl ClipboardService {
    pub async fn start(options: Options) -> Result<(Self, Clipboard)> {
        Self::start_with(options, backend::open).await
    }
    pub(crate) async fn start_with<F>(options: Options, factory: F) -> Result<(Self, Clipboard)>
    where
        F: FnOnce(&Options) -> Result<Box<dyn Backend>> + Send + 'static,
    {
        options.validate()?;
        let wake = Arc::new(Wake::new()?);
        let stop = Arc::new(AtomicBool::new(false));
        let (requests, receiver) = mpsc::channel(options.queue_capacity);
        let (snapshots, snapshot_receiver) = watch::channel(None);
        let (status, status_receiver) = watch::channel(ServiceStatus::Starting);
        let (ready, readiness) = oneshot::channel();
        let worker_wake = wake.clone();
        let worker_stop = stop.clone();
        let executor = tokio::runtime::Handle::current();
        let thread = thread::Builder::new()
            .name("nooboard-clipboard".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let backend = match factory(&options) {
                        Ok(backend) => backend,
                        Err(error) => {
                            let _ = ready.send(Err(error.clone()));
                            return Err(error);
                        }
                    };
                    status.send_replace(ServiceStatus::Ready);
                    if ready.send(Ok(())).is_err() {
                        return Ok(());
                    }
                    runtime::run(
                        backend,
                        options,
                        runtime::Channels {
                            requests: receiver,
                            snapshots,
                            status: status.clone(),
                        },
                        worker_wake,
                        worker_stop,
                        executor,
                    )
                }))
                .unwrap_or_else(|_| Err(Error::backend("worker", "native worker panicked")));
                status.send_replace(ServiceStatus::Stopped {
                    error: result.as_ref().err().cloned(),
                });
                result
            })
            .map_err(|e| Error::backend("start worker", e))?;
        let service = Self {
            stop,
            wake: wake.clone(),
            thread: Some(thread),
        };
        readiness.await.map_err(|_| Error::Stopped)??;
        Ok((
            service,
            Clipboard {
                requests,
                wake,
                snapshots: snapshot_receiver,
                status: status_receiver,
            },
        ))
    }
    pub async fn shutdown(mut self) -> Result<()> {
        self.signal_stop();
        let Some(thread) = self.thread.take() else {
            return Ok(());
        };
        tokio::task::spawn_blocking(move || thread.join())
            .await
            .map_err(|e| Error::backend("join worker", e))?
            .map_err(|_| Error::backend("join worker", "worker panicked"))?
    }
    fn signal_stop(&self) {
        self.stop.store(true, Ordering::Release);
        self.wake.notify();
    }
}
impl Drop for ClipboardService {
    fn drop(&mut self) {
        self.signal_stop();
    }
}
