pub use crate::{
    connection::model::{Connect, ConnectScope, ConnectionState, HostKey, SessionStatus},
    credentials::{CredentialStore, Password},
    error::{Error, Result},
    event::Event,
    options::Options,
    sftp::{
        directory::model::{DirectoryBatch, DirectoryEntry, FileKind},
        path::RemotePath,
        transfer::model::{Transfer, TransferSpec, TransferState},
    },
    terminal::model::{PtySize, TerminalChunk, TerminalReplay},
};
use crate::{runtime, sftp, terminal};
use serde::Serialize;
use tokio::{
    sync::{broadcast, mpsc, watch},
    task::JoinHandle,
};

#[derive(Clone, Debug, Default, Serialize)]
pub struct Snapshot {
    pub sessions: Vec<SessionStatus>,
    pub transfers: Vec<Transfer>,
}

/// The sole lifetime owner. Dropping a UI handle never shuts this service down.
pub struct SshService {
    pub(crate) stop: watch::Sender<bool>,
    pub(crate) task: Option<JoinHandle<Result<()>>>,
}
impl SshService {
    pub async fn start(options: Options) -> Result<(Self, Ssh, SshEvents)> {
        runtime::start(options)
    }
    pub async fn shutdown(mut self) -> Result<()> {
        self.stop.send_replace(true);
        match self.task.take() {
            Some(task) => task.await.map_err(|_| Error::Internal)?,
            None => Ok(()),
        }
    }
}
impl Drop for SshService {
    fn drop(&mut self) {
        self.stop.send_replace(true);
    }
}

#[derive(Clone)]
pub struct Ssh {
    pub(crate) requests: mpsc::Sender<runtime::Request>,
    pub(crate) context: runtime::Context,
    pub(crate) terminal: terminal::runtime::Handle,
    pub(crate) files: sftp::runtime::Handle,
    pub(crate) directory: sftp::directory::runtime::Handle,
    pub(crate) transfer: sftp::transfer::runtime::Handle,
}
impl Ssh {
    pub fn is_running(&self) -> bool {
        !*self.context.stop.borrow()
    }
    pub fn snapshot(&self) -> Snapshot {
        self.context.state.borrow().clone()
    }
    pub fn subscribe(&self) -> watch::Receiver<Snapshot> {
        self.context.state.subscribe()
    }
    /// Subscribe before taking a replay; discard live chunks below replay.end.
    /// On broadcast lag, request a replay and respect its explicit gap marker.
    pub fn subscribe_terminal(&self) -> broadcast::Receiver<TerminalChunk> {
        self.context.output.subscribe()
    }
    pub fn terminal_replay(&self, id: &str, after: u64) -> Result<TerminalReplay> {
        self.context.output.replay(id, after)
    }
    pub async fn connect(&self, options: Connect) -> Result<String> {
        runtime::request(&self.requests, &self.context.stop, |reply| {
            runtime::Request::Connect { options, reply }
        })
        .await
    }
    pub async fn decide_host_key(&self, request_id: String, accept: bool) -> Result<()> {
        self.context.keys.decide(&request_id, accept)
    }
    pub async fn write_terminal(&self, id: String, bytes: Vec<u8>) -> Result<()> {
        self.terminal.input(id, bytes).await
    }
    pub async fn resize_terminal(&self, id: String, size: PtySize) -> Result<()> {
        size.validate()?;
        self.terminal.resize(id, size).await
    }
    pub async fn close_terminal(&self, id: String) -> Result<()> {
        self.terminal.close(id).await
    }
    /// Closing browsing does not require calling this: file sessions remain alive
    /// until explicitly released or the service shuts down.
    pub async fn close_files(&self, id: String) -> Result<()> {
        self.files.close(id).await
    }
    /// Release a finished session and its terminal replay buffer. Active
    /// terminals, file connections and transfers make this return Busy.
    pub async fn forget_session(&self, id: String) -> Result<()> {
        runtime::request(&self.requests, &self.context.stop, |reply| {
            runtime::Request::Forget { id, reply }
        })
        .await
    }
    pub async fn validate_directory(
        &self,
        id: String,
        base: RemotePath,
        input: String,
    ) -> Result<RemotePath> {
        self.directory.validate(id, base, input).await
    }
    pub async fn read_directory(&self, id: String, path: RemotePath) -> Result<DirectoryBatch> {
        self.directory.open(id, path).await
    }
    pub async fn next_directory_batch(&self, cursor: String) -> Result<DirectoryBatch> {
        self.directory.next(cursor).await
    }
    pub async fn cancel_directory(&self, query: String) -> Result<()> {
        self.directory.cancel(query).await
    }
    pub async fn transfer(&self, session_id: String, spec: TransferSpec) -> Result<String> {
        self.transfer.submit(session_id, spec).await
    }
    pub async fn cancel_transfer(&self, id: String) -> Result<()> {
        self.transfer.cancel(id).await
    }
    pub async fn retry_transfer(&self, id: String) -> Result<String> {
        self.transfer.retry(id).await
    }
    pub async fn forget_transfer(&self, id: String) -> Result<()> {
        self.transfer.forget(id).await
    }
}
/// Reliable decisions have exactly one consumer and never share a lossy progress channel.
pub struct SshEvents {
    pub(crate) receiver: mpsc::Receiver<Event>,
    pub(crate) stopped: watch::Receiver<bool>,
}
impl SshEvents {
    pub async fn next_event(&mut self) -> Option<Event> {
        if *self.stopped.borrow() {
            return None;
        }
        tokio::select! { biased;
            _ = self.stopped.changed() => None,
            event = self.receiver.recv() => event,
        }
    }
}
