use crate::{Error, Event, HistoryEntry, Options, PairRequest, Result, Settings, Status};
use tokio::{
    sync::{broadcast, mpsc, oneshot, watch},
    task::JoinHandle,
};

pub(crate) enum Command {
    Settings(Settings),
    Pair(PairRequest),
    Unpair,
    Send,
    History {
        contains: String,
        limit: u32,
        offset: u32,
    },
    CopyHistory(i64),
    DeleteHistory(i64),
    ClearHistory,
    Stop,
}
pub(crate) enum Reply {
    Done,
    Sequence(u64),
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
    pub(crate) events: broadcast::Sender<Event>,
    pub(crate) certificate: Vec<u8>,
    pub(crate) task: Option<JoinHandle<Result<()>>>,
}
impl App {
    pub async fn start(options: Options) -> Result<Self> {
        crate::bootstrap::start(options).await
    }
    pub fn status(&self) -> Status {
        self.status.borrow().clone()
    }
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.events.subscribe()
    }
    /// Public certificate only; safe to copy to the other device for pairing.
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
    pub async fn set_settings(&self, settings: Settings) -> Result<()> {
        self.request(Command::Settings(settings)).await?;
        Ok(())
    }
    pub async fn pair(&self, request: PairRequest) -> Result<()> {
        self.request(Command::Pair(request)).await?;
        Ok(())
    }
    pub async fn unpair(&self) -> Result<()> {
        self.request(Command::Unpair).await?;
        Ok(())
    }
    /// Returns a sequence after transport write; Event::Applied confirms remote application.
    pub async fn send_current(&self) -> Result<u64> {
        match self.request(Command::Send).await? {
            Reply::Sequence(n) => Ok(n),
            _ => Err(Error::Stopped),
        }
    }
    pub async fn history(
        &self,
        contains: String,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<HistoryEntry>> {
        match self
            .request(Command::History {
                contains,
                limit,
                offset,
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
