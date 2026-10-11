use crate::{
    runtime::{self, Context, Reply, request},
    sftp::runtime::Handle as Files,
    *,
};
use std::{collections::HashMap, sync::Arc};
use tokio::{
    sync::{Semaphore, mpsc, watch},
    task::JoinSet,
};

enum Request {
    Submit {
        session: String,
        spec: TransferSpec,
        reply: Reply<String>,
    },
    Cancel {
        id: String,
        reply: Reply<()>,
    },
    Retry {
        id: String,
        reply: Reply<String>,
    },
    Forget {
        id: String,
        reply: Reply<()>,
    },
}
#[derive(Clone)]
pub(crate) struct Handle {
    sender: mpsc::Sender<Request>,
    context: Context,
}
impl Handle {
    pub async fn submit(&self, session: String, spec: TransferSpec) -> Result<String> {
        request(&self.sender, &self.context.stop, |reply| Request::Submit {
            session,
            spec,
            reply,
        })
        .await
    }
    pub async fn cancel(&self, id: String) -> Result<()> {
        request(&self.sender, &self.context.stop, |reply| Request::Cancel {
            id,
            reply,
        })
        .await
    }
    pub async fn retry(&self, id: String) -> Result<String> {
        request(&self.sender, &self.context.stop, |reply| Request::Retry {
            id,
            reply,
        })
        .await
    }
    pub async fn forget(&self, id: String) -> Result<()> {
        request(&self.sender, &self.context.stop, |reply| Request::Forget {
            id,
            reply,
        })
        .await
    }
}
pub(crate) struct Runtime {
    inbox: mpsc::Receiver<Request>,
    context: Context,
    files: Files,
}
pub(crate) fn create(context: Context, files: Files) -> (Handle, Runtime) {
    let (sender, inbox) = mpsc::channel(32);
    (
        Handle {
            sender,
            context: context.clone(),
        },
        Runtime {
            inbox,
            context,
            files,
        },
    )
}
pub(super) struct Progress {
    context: Context,
    id: String,
    cancel: watch::Receiver<bool>,
}
impl Progress {
    pub fn check_cancelled(&self) -> Result<()> {
        if *self.cancel.borrow() || *self.context.stop.borrow() {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }
    pub fn bytes(&self, value: u64) {
        self.context.transfer(&self.id, |t| t.transferred = value);
    }
    pub fn total(&self, value: u64) {
        self.context.transfer(&self.id, |t| t.total = Some(value));
    }
    pub fn temporary(&self, path: String) {
        self.context
            .transfer(&self.id, |t| t.temporary_path = Some(path));
    }
    pub fn cleaned(&self) {
        self.context.transfer(&self.id, |t| t.temporary_path = None);
    }
    pub fn target(&self, path: String) {
        self.context.transfer(&self.id, |t| t.target = Some(path));
    }
    pub fn finalizing(&self) {
        self.context
            .transfer(&self.id, |t| t.state = TransferState::Finalizing);
    }
}
impl Runtime {
    pub async fn run(mut self) -> Result<()> {
        let mut stop = self.context.stop.clone();
        let mut tasks: HashMap<String, watch::Sender<bool>> = HashMap::new();
        let mut jobs = JoinSet::new();
        let permits = Arc::new(Semaphore::new(self.context.options.transfer_concurrency));
        loop {
            tokio::select! { biased;
                _ = stop.changed() => break,
                result = jobs.join_next(), if !jobs.is_empty() => {
                    let id = result.ok_or(Error::Internal)?.map_err(|_| Error::Internal)?; tasks.remove(&id);
                },
                Some(message) = self.inbox.recv() => {
                    let (session, spec, reply) = match message {
                        Request::Forget { id, reply } => {
                            if self.context.state.borrow().transfers.iter().any(|t| t.id == id && !t.state.finished()) { let _ = reply.send(Err(Error::Busy)); }
                            else { self.context.state.send_modify(|s| s.transfers.retain(|t| t.id != id)); let _ = reply.send(Ok(())); }
                            continue;
                        },
                        Request::Cancel { id, reply } => { if let Some(cancel) = tasks.get(&id) { cancel.send_replace(true); } let _ = reply.send(Ok(())); continue; },
                        Request::Submit { session, spec, reply } => (session, spec, reply),
                        Request::Retry { id, reply } => {
                            let original = self.context.state.borrow().transfers.iter().find(|t| t.id == id).cloned();
                            match original {
                                Some(t) if matches!(t.state, TransferState::Failed | TransferState::Cancelled) => (t.session_id, t.spec, reply),
                                Some(_) => { let _ = reply.send(Err(Error::Busy)); continue; },
                                None => { let _ = reply.send(Err(Error::NotFound)); continue; },
                            }
                        },
                    };
                    if self.context.state.borrow().transfers.len() >= self.context.options.max_transfers { let _ = reply.send(Err(Error::Limit)); continue; }
                    if !self.context.state.borrow().sessions.iter().any(|s| s.id == session) { let _ = reply.send(Err(Error::NotFound)); continue; }
                    let valid = match &spec {
                        TransferSpec::Upload { local, remote_directory } => local.is_absolute() && absolute(remote_directory),
                        TransferSpec::Download { remote, local_directory } => absolute(remote) && local_directory.is_absolute(),
                    };
                    if !valid { let _ = reply.send(Err(Error::InvalidPath)); continue; }
                    let id = runtime::id();
                    let (cancel, cancelled) = watch::channel(false);
                    tasks.insert(id.clone(), cancel);
                    self.context.state.send_modify(|s| {
                        let device_id = s.sessions.iter().find(|s| s.id == session).expect("session checked").device_id.clone();
                        s.transfers.push(Transfer { id: id.clone(), session_id: session.clone(), device_id, spec: spec.clone(), state: TransferState::Queued, transferred: 0, total: None, target: None, temporary_path: None, error: None });
                    });
                    let progress = Progress { context: self.context.clone(), id: id.clone(), cancel: cancelled };
                    let files = self.files.clone();
                    let permits = permits.clone();
                    let _ = reply.send(Ok(id.clone()));
                    jobs.spawn(async move { execute(&session, spec, progress, files, permits).await; id });
                },
            }
        }
        for cancel in tasks.values() {
            cancel.send_replace(true);
        }
        while let Some(result) = jobs.join_next().await {
            result.map_err(|_| Error::Internal)?;
        }
        Ok(())
    }
}
fn absolute(path: &RemotePath) -> bool {
    let value = path.as_str();
    value.starts_with('/')
        || (value.as_bytes().get(1) == Some(&b':') && value.as_bytes().get(2) == Some(&b'/'))
}
async fn execute(
    session: &str,
    spec: TransferSpec,
    progress: Progress,
    files: Files,
    permits: Arc<Semaphore>,
) {
    let run = async {
        let mut snapshots = progress.context.state.subscribe();
        let client = loop {
            progress.check_cancelled()?;
            if let Ok(client) = files.get(session) {
                break client;
            }
            let state = snapshots
                .borrow_and_update()
                .sessions
                .iter()
                .find(|s| s.id == session)
                .map(|s| s.files.clone());
            match state {
                Some(ConnectionState::Connecting) => progress.context.transfer(&progress.id, |t| {
                    t.state = TransferState::WaitingForConnection
                }),
                Some(ConnectionState::Failed(e)) => return Err(e),
                _ => return Err(Error::SftpUnavailable),
            }
            let mut cancelled = progress.cancel.clone();
            let mut stop = progress.context.stop.clone();
            tokio::select! { _ = snapshots.changed() => {}, _ = cancelled.changed() => return Err(Error::Cancelled), _ = stop.changed() => return Err(Error::Cancelled) }
        };
        let mut cancelled = progress.cancel.clone();
        let mut stop = progress.context.stop.clone();
        let _permit = tokio::select! {
            result = permits.acquire() => result.map_err(|_| Error::Stopped)?,
            _ = cancelled.changed() => return Err(Error::Cancelled),
            _ = stop.changed() => return Err(Error::Cancelled),
        };
        progress.check_cancelled()?;
        progress
            .context
            .transfer(&progress.id, |t| t.state = TransferState::Running);
        match spec {
            TransferSpec::Upload {
                local,
                remote_directory,
            } => super::upload::run(&client, &local, &remote_directory, &progress).await,
            TransferSpec::Download {
                remote,
                local_directory,
            } => super::download::run(&client, &remote, &local_directory, &progress).await,
        }
    };
    let result: Result<()> = run.await;
    progress.context.transfer(&progress.id, |t| {
        t.state = match &result {
            Ok(()) => TransferState::Succeeded,
            Err(Error::Cancelled) => TransferState::Cancelled,
            Err(Error::Unconfirmed) => TransferState::Unconfirmed,
            Err(_) => TransferState::Failed,
        };
        t.error = result.err();
    });
}
