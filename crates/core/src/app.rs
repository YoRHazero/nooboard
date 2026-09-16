use crate::{
    Error, Event, HistoryEntry, Options, PeerSettings, Result, Settings, Status, VerifiedPeer,
};
use tokio::{
    sync::{broadcast, mpsc, oneshot, watch},
    task::JoinHandle,
};

pub(crate) enum Command {
    RefreshDiscovery,
    BeginPairing {
        address: String,
        expected: Option<String>,
    },
    AcceptPairing(String),
    SubmitPairingCode {
        id: String,
        code: String,
    },
    DismissPairing(String),
    Settings(Settings),
    TrustPeer(VerifiedPeer),
    Unpair(String),
    ConfigurePeer(String, PeerSettings),
    SelectTargets(Vec<String>),
    Send(Option<Vec<String>>),
    History {
        contains: String,
        limit: u32,
        offset: u32,
        local: Option<bool>,
    },
    CopyHistory(i64),
    DeleteHistory(i64),
    ClearHistory,
    Stop,
}
pub(crate) enum Reply {
    Done,
    MessageId(nooboard_network::MessageId),
    History(Vec<HistoryEntry>),
}
pub(crate) struct Request {
    pub command: Command,
    pub response: oneshot::Sender<Result<Reply>>,
}

/// Owns the backend lifetime. Dropping App stops background work; prefer shutdown().
pub struct App {
    pub(crate) commands: mpsc::Sender<Request>,
    pub(crate) status: watch::Receiver<Status>,
    pub(crate) snapshots: watch::Receiver<crate::AppSnapshot>,
    pub(crate) events: broadcast::Sender<Event>,
    #[cfg(any(test, feature = "diagnostics"))]
    pub(crate) certificate: Vec<u8>,
    pub(crate) task: Option<JoinHandle<Result<()>>>,
}
impl App {
    pub async fn start(options: Options) -> Result<Self> {
        crate::bootstrap::start(options).await
    }
    pub fn is_running(&self) -> bool {
        self.task.as_ref().is_some_and(|task| !task.is_finished())
    }
    pub fn status(&self) -> Status {
        self.status.borrow().clone()
    }
    pub fn snapshot(&self) -> crate::AppSnapshot {
        self.snapshots.borrow().clone()
    }
    pub fn subscribe_snapshots(&self) -> watch::Receiver<crate::AppSnapshot> {
        self.snapshots.clone()
    }
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.events.subscribe()
    }
    /// Public certificate for isolated transport diagnostics.
    #[cfg(any(test, feature = "diagnostics"))]
    pub fn certificate(&self) -> &[u8] {
        &self.certificate
    }
    async fn request(&self, command: Command) -> Result<Reply> {
        let (response, result) = oneshot::channel();
        self.commands
            .send(Request { command, response })
            .await
            .map_err(|_| Error::Stopped)?;
        result.await.map_err(|_| Error::Stopped)?
    }
    pub async fn refresh_discovery(&self) -> Result<()> {
        self.request(Command::RefreshDiscovery).await?;
        Ok(())
    }
    pub async fn begin_pairing(&self, address: String, expected: Option<String>) -> Result<()> {
        self.request(Command::BeginPairing { address, expected })
            .await?;
        Ok(())
    }
    pub async fn accept_pairing(&self, id: String) -> Result<()> {
        self.request(Command::AcceptPairing(id)).await?;
        Ok(())
    }
    pub async fn submit_pairing_code(&self, id: String, code: String) -> Result<()> {
        self.request(Command::SubmitPairingCode { id, code })
            .await?;
        Ok(())
    }
    pub async fn dismiss_pairing(&self, id: String) -> Result<()> {
        self.request(Command::DismissPairing(id)).await?;
        Ok(())
    }
    pub async fn set_settings(&self, settings: Settings) -> Result<()> {
        self.request(Command::Settings(settings)).await?;
        Ok(())
    }
    #[cfg(any(test, feature = "diagnostics"))]
    pub(crate) async fn trust_peer(&self, request: VerifiedPeer) -> Result<String> {
        let id = nooboard_network::noob_id(&request.certificate)?;
        self.request(Command::TrustPeer(request)).await?;
        Ok(id)
    }
    pub async fn unpair(&self, noob_id: String) -> Result<()> {
        self.request(Command::Unpair(noob_id)).await?;
        Ok(())
    }
    pub async fn configure_peer(&self, noob_id: String, settings: PeerSettings) -> Result<()> {
        self.request(Command::ConfigurePeer(noob_id, settings))
            .await?;
        Ok(())
    }
    pub async fn select_targets(&self, targets: Vec<String>) -> Result<()> {
        self.request(Command::SelectTargets(targets)).await?;
        Ok(())
    }
    /// Enqueues one immutable snapshot for the remembered manual targets.
    /// Track Event::Transfer for each target's write and application result.
    pub async fn send_current(&self) -> Result<nooboard_network::MessageId> {
        self.send_request(None).await
    }
    /// Sends to selected peers and remembers that selection independently of automatic routing.
    pub async fn send_to(&self, targets: Vec<String>) -> Result<nooboard_network::MessageId> {
        self.send_request(Some(targets)).await
    }
    async fn send_request(
        &self,
        targets: Option<Vec<String>>,
    ) -> Result<nooboard_network::MessageId> {
        match self.request(Command::Send(targets)).await? {
            Reply::MessageId(id) => Ok(id),
            _ => Err(Error::Stopped),
        }
    }
    pub async fn history(
        &self,
        contains: String,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<HistoryEntry>> {
        self.history_filtered(contains, None, limit, offset).await
    }
    /// Filter before pagination: Some(true) is local, Some(false) is received text.
    pub async fn history_filtered(
        &self,
        contains: String,
        local: Option<bool>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<HistoryEntry>> {
        match self
            .request(Command::History {
                contains,
                limit,
                offset,
                local,
            })
            .await?
        {
            Reply::History(rows) => Ok(rows),
            _ => Err(Error::Stopped),
        }
    }
    pub async fn copy_history(&self, id: i64) -> Result<()> {
        self.request(Command::CopyHistory(id)).await?;
        Ok(())
    }
    pub async fn delete_history(&self, id: i64) -> Result<()> {
        self.request(Command::DeleteHistory(id)).await?;
        Ok(())
    }
    pub async fn clear_history(&self) -> Result<()> {
        self.request(Command::ClearHistory).await?;
        Ok(())
    }
    pub async fn shutdown(mut self) -> Result<()> {
        self.request(Command::Stop).await?;
        if let Some(task) = self.task.take() {
            task.await.map_err(|_| Error::Stopped)??;
        }
        Ok(())
    }
}
impl Drop for App {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
