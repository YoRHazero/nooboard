use crate::{
    BackendConfig, Error, ErrorKind, HistoryEntry, HistoryId, HistoryPage, HistoryQuery, Options,
    RecordHistory, RecordOutcome, Result, Retention, SettingsChanges,
    backend::{self, contract::Backend},
    model::validate_key,
    runtime::{
        self,
        request::{Reply, Request},
    },
};
use tokio::{
    sync::{mpsc, oneshot, watch},
    task::JoinHandle,
};

/// Owns the service lifetime. Drop signals stop; `shutdown` waits for cleanup.
pub struct StorageService {
    stop: watch::Sender<bool>,
    task: Option<JoinHandle<Result<()>>>,
}
impl StorageService {
    /// Requires a running Tokio runtime. Returns only after the backend is ready
    /// and its schema migration has committed.
    pub async fn start(config: BackendConfig, options: Options) -> Result<(Self, Storage)> {
        options.validate()?;
        let backend = backend::open(config).await?;
        Ok(Self::spawn(backend, options))
    }
    pub(crate) fn spawn(backend: Box<dyn Backend>, options: Options) -> (Self, Storage) {
        let (requests, receiver) = mpsc::channel(options.queue_capacity);
        let (stop, stopped) = watch::channel(false);
        let task = tokio::spawn(runtime::run(backend, receiver, stopped.clone()));
        (
            Self {
                stop,
                task: Some(task),
            },
            Storage { requests, stopped },
        )
    }
    /// Reject new/queued work and wait for any active operation and backend cleanup.
    /// Already-started writes may commit successfully during shutdown.
    pub async fn shutdown(mut self) -> Result<()> {
        self.stop.send_replace(true);
        match self.task.take() {
            Some(task) => task
                .await
                .map_err(|e| Error::caused_by(ErrorKind::Internal, "join storage runtime", e))?,
            None => Ok(()),
        }
    }
}
impl Drop for StorageService {
    fn drop(&mut self) {
        self.stop.send_replace(true);
    }
}

/// Cloneable message proxy. No database connection or SQL is exposed.
#[derive(Clone)]
pub struct Storage {
    requests: mpsc::Sender<Request>,
    stopped: watch::Receiver<bool>,
}
impl Storage {
    pub fn history(&self) -> History {
        History(self.clone())
    }
    pub fn settings(&self) -> Settings {
        Settings(self.clone())
    }
    async fn request<T>(&self, make: impl FnOnce(Reply<T>) -> Request) -> Result<T> {
        let mut stopped = self.stopped.clone();
        if *stopped.borrow() {
            return Err(Error::stopped());
        }
        let (reply, response) = oneshot::channel();
        let request = make(reply);
        tokio::select! {
            biased;
            _ = stopped.changed() => return Err(Error::stopped()),
            sent = self.requests.send(request) => sent.map_err(|_| Error::stopped())?,
        }
        response.await.map_err(|_| Error::stopped())?
    }
}

#[derive(Clone)]
pub struct History(Storage);
impl History {
    /// Exact text equality refreshes the existing entry without changing its ID.
    /// Recording and retention commit together; a backdated entry may be pruned.
    pub async fn record(&self, input: RecordHistory) -> Result<RecordOutcome> {
        input.validate()?;
        self.0
            .request(|reply| Request::Record { input, reply })
            .await
    }
    pub async fn query(&self, input: HistoryQuery) -> Result<HistoryPage> {
        input.validate()?;
        self.0
            .request(|reply| Request::Query { input, reply })
            .await
    }
    pub async fn get(&self, id: HistoryId) -> Result<Option<HistoryEntry>> {
        self.0.request(|reply| Request::Get { id, reply }).await
    }
    pub async fn delete(&self, id: HistoryId) -> Result<bool> {
        self.0.request(|reply| Request::Delete { id, reply }).await
    }
    pub async fn prune(&self, retention: Retention) -> Result<u64> {
        self.0
            .request(|reply| Request::Prune { retention, reply })
            .await
    }
    /// Logical deletion, not a physical secure-erasure guarantee.
    pub async fn clear(&self) -> Result<u64> {
        self.0.request(|reply| Request::Clear { reply }).await
    }
}

#[derive(Clone)]
pub struct Settings(Storage);
impl Settings {
    pub async fn get(&self, key: impl Into<String>) -> Result<Option<String>> {
        let mut values = self.get_many(vec![key.into()]).await?;
        Ok(values.pop().expect("one key produces one result"))
    }
    /// Reads one snapshot; absent values are None, including for repeated keys.
    pub async fn get_many(&self, keys: Vec<String>) -> Result<Vec<Option<String>>> {
        if keys.len() > 1024 {
            return Err(Error::invalid("settings batch size"));
        }
        for key in &keys {
            validate_key(key)?;
        }
        self.0
            .request(|reply| Request::ReadSettings { keys, reply })
            .await
    }
    /// Atomically apply all changes. Duplicate/overlapping keys are rejected.
    pub async fn apply(&self, changes: SettingsChanges) -> Result<()> {
        changes.validate()?;
        self.0
            .request(|reply| Request::ApplySettings { changes, reply })
            .await
    }
}
