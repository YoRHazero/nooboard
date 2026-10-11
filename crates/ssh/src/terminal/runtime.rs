use crate::{
    connection::client::Connection,
    runtime::{Context, Reply, request},
    *,
};
use std::collections::HashMap;
use tokio::{sync::mpsc, task::JoinSet};

pub(super) enum Command {
    Input(Vec<u8>),
    Resize(PtySize),
    Close,
}
enum Request {
    Attach {
        id: String,
        connection: Connection,
        size: PtySize,
        reply: Reply<()>,
    },
    Command {
        id: String,
        command: Command,
        reply: Reply<()>,
    },
}
#[derive(Clone)]
pub(crate) struct Handle {
    sender: mpsc::Sender<Request>,
    context: Context,
}
impl Handle {
    pub async fn attach(&self, id: String, connection: Connection, size: PtySize) -> Result<()> {
        request(&self.sender, &self.context.stop, |reply| Request::Attach {
            id,
            connection,
            size,
            reply,
        })
        .await
    }
    async fn command(&self, id: String, command: Command) -> Result<()> {
        request(&self.sender, &self.context.stop, |reply| Request::Command {
            id,
            command,
            reply,
        })
        .await
    }
    pub async fn input(&self, id: String, bytes: Vec<u8>) -> Result<()> {
        if bytes.len() > 64 * 1024 {
            return Err(Error::Limit);
        }
        self.command(id, Command::Input(bytes)).await
    }
    pub async fn resize(&self, id: String, size: PtySize) -> Result<()> {
        self.command(id, Command::Resize(size)).await
    }
    pub async fn close(&self, id: String) -> Result<()> {
        self.command(id, Command::Close).await
    }
}
pub(crate) struct Runtime {
    inbox: mpsc::Receiver<Request>,
    context: Context,
}
pub(crate) fn create(context: Context) -> (Handle, Runtime) {
    let (sender, inbox) = mpsc::channel(64);
    (
        Handle {
            sender,
            context: context.clone(),
        },
        Runtime { inbox, context },
    )
}
impl Runtime {
    pub async fn run(mut self) -> Result<()> {
        let mut stop = self.context.stop.clone();
        let mut sessions: HashMap<String, mpsc::Sender<Command>> = HashMap::new();
        let mut jobs = JoinSet::new();
        loop {
            tokio::select! { biased;
                _ = stop.changed() => break,
                result = jobs.join_next(), if !jobs.is_empty() => {
                    let id = result.ok_or(Error::Internal)?.map_err(|_| Error::Internal)?;
                    sessions.remove(&id);
                },
                Some(message) = self.inbox.recv() => match message {
                    Request::Attach { id, connection, size, reply } => {
                        let (sender, inbox) = mpsc::channel(32);
                        sessions.insert(id.clone(), sender);
                        let context = self.context.clone();
                        jobs.spawn(async move { super::session::run(&id, connection, size, inbox, context).await; id });
                        let _ = reply.send(Ok(()));
                    },
                    Request::Command { id, command, reply } => {
                        let result = match sessions.get(&id) {
                            Some(sender) => sender.try_send(command).map_err(|e| match e { mpsc::error::TrySendError::Full(_) => Error::Busy, _ => Error::NotFound }),
                            None if matches!(command, Command::Close) => {
                                self.context.session(&id, |s| s.terminal = ConnectionState::Closed); Ok(())
                            },
                            None => Err(Error::NotFound),
                        };
                        let _ = reply.send(result);
                    },
                }
            }
        }
        while let Some(result) = jobs.join_next().await {
            result.map_err(|_| Error::Internal)?;
        }
        Ok(())
    }
}
