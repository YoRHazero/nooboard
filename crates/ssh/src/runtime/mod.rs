use crate::{connection::client, sftp, terminal, *};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinSet;

pub(crate) type Reply<T> = oneshot::Sender<Result<T>>;
pub(crate) async fn request<M, T>(
    sender: &mpsc::Sender<M>,
    stop: &watch::Receiver<bool>,
    make: impl FnOnce(Reply<T>) -> M,
) -> Result<T> {
    let (tx, rx) = oneshot::channel();
    let mut stop = stop.clone();
    if *stop.borrow() {
        return Err(Error::Stopped);
    }
    tokio::select! { biased; _ = stop.changed() => return Err(Error::Stopped), result = sender.send(make(tx)) => result.map_err(|_| Error::Stopped)? }
    tokio::select! { biased; _ = stop.changed() => Err(Error::Stopped), result = rx => result.map_err(|_| Error::Stopped)? }
}
pub(crate) fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[derive(Clone)]
pub(crate) struct Context {
    pub options: Arc<Options>,
    pub stop: watch::Receiver<bool>,
    pub state: watch::Sender<Snapshot>,
    pub keys: client::KeyDecisions,
    pub output: terminal::output::Output,
}
impl Context {
    pub fn session(&self, id: &str, update: impl FnOnce(&mut SessionStatus)) {
        self.state.send_modify(|s| {
            if let Some(session) = s.sessions.iter_mut().find(|s| s.id == id) {
                update(session);
            }
        });
    }
    pub fn file_failure(&self, id: &str, error: Error) {
        self.session(id, |s| {
            if s.files != ConnectionState::Closed {
                s.files = ConnectionState::Failed(error);
            }
        });
    }
    pub fn transfer(&self, id: &str, update: impl FnOnce(&mut Transfer)) {
        self.state.send_modify(|s| {
            if let Some(task) = s.transfers.iter_mut().find(|t| t.id == id) {
                update(task);
            }
        });
    }
}
pub(crate) enum Request {
    Connect {
        options: Connect,
        reply: Reply<String>,
    },
    Forget {
        id: String,
        reply: Reply<()>,
    },
}

pub(crate) fn start(options: Options) -> Result<(SshService, Ssh, SshEvents)> {
    options.validate()?;
    let (stop, stopped) = watch::channel(false);
    let (events, receiver) = mpsc::channel(32);
    let (state, _) = watch::channel(Snapshot::default());
    let context = Context {
        output: terminal::output::Output::new(options.terminal_replay_bytes),
        keys: client::KeyDecisions::new(events),
        options: Arc::new(options),
        stop: stopped,
        state,
    };
    let (terminal, terminal_runtime) = terminal::runtime::create(context.clone());
    let (files, directory, transfer, file_runtime) = sftp::runtime::create(context.clone());
    let (requests, inbox) = mpsc::channel(32);
    let ssh = Ssh {
        requests,
        context: context.clone(),
        terminal,
        files,
        directory,
        transfer,
    };
    let handles = ssh.clone();
    let event_stop = context.stop.clone();
    let signal = stop.clone();
    let task = tokio::spawn(async move {
        let mut workers = JoinSet::new();
        workers.spawn(terminal_runtime.run());
        workers.spawn(file_runtime.run());
        workers.spawn(run(inbox, handles));
        let mut stopped = context.stop.clone();
        let result = tokio::select! { biased;
            _ = stopped.changed() => Ok(()),
            result = workers.join_next() => match result { Some(Ok(Err(e))) => Err(e), Some(Ok(Ok(()))) if *stopped.borrow() => Ok(()), _ => Err(Error::Internal) },
        };
        signal.send_replace(true);
        // Every feature drains its owned workers; connection-scoped failures are
        // represented in status and cannot bring unrelated sessions down.
        while let Some(result) = workers.join_next().await {
            result.map_err(|_| Error::Internal)??;
        }
        context.keys.clear();
        result
    });
    Ok((
        SshService {
            stop,
            task: Some(task),
        },
        ssh,
        SshEvents {
            receiver,
            stopped: event_stop,
        },
    ))
}
async fn run(mut inbox: mpsc::Receiver<Request>, ssh: Ssh) -> Result<()> {
    let mut stop = ssh.context.stop.clone();
    let mut jobs = JoinSet::new();
    loop {
        tokio::select! { biased;
            _ = stop.changed() => break,
            result = jobs.join_next(), if !jobs.is_empty() => { result.ok_or(Error::Internal)?.map_err(|_| Error::Internal)?; },
            Some(message) = inbox.recv() => dispatch(message, &ssh, &mut jobs).await,
        }
    }
    // Initializers observe stop, including during host-key approval.
    while let Some(result) = jobs.join_next().await {
        result.map_err(|_| Error::Internal)?;
    }
    Ok(())
}
async fn dispatch(message: Request, ssh: &Ssh, jobs: &mut JoinSet<()>) {
    match message {
        Request::Forget { id, reply } => {
            let active = {
                let state = ssh.context.state.borrow();
                state.sessions.iter().find(|s| s.id == id).is_some_and(|s| {
                    !s.attempt_finished
                        || matches!(
                            s.terminal,
                            ConnectionState::Connecting | ConnectionState::Ready
                        )
                        || matches!(
                            s.files,
                            ConnectionState::Connecting | ConnectionState::Ready
                        )
                }) || state
                    .transfers
                    .iter()
                    .any(|t| t.session_id == id && !t.state.finished())
            };
            let result = if active {
                Err(Error::Busy)
            } else {
                ssh.files.close(id.clone()).await
            };
            if result.is_ok() {
                ssh.context
                    .state
                    .send_modify(|s| s.sessions.retain(|s| s.id != id));
                ssh.context.output.remove(&id);
            }
            let _ = reply.send(result);
        }
        Request::Connect { options, reply } => {
            if reply.is_closed() {
                return;
            }
            let validation = options.size.validate().and_then(|_| {
                if options.addresses.is_empty()
                    || options.addresses.len() > 64
                    || options.username.is_empty()
                    || options.username.len() > 256
                    || options.addresses.iter().any(|a| a.port() == 0)
                {
                    Err(Error::InvalidPath)
                } else if ssh.snapshot().sessions.len() >= ssh.context.options.max_sessions {
                    Err(Error::Limit)
                } else {
                    Ok(())
                }
            });
            if let Err(e) = validation {
                let _ = reply.send(Err(e));
                return;
            }
            let id = id();
            ssh.context.state.send_modify(|s| {
                s.sessions.push(SessionStatus {
                    id: id.clone(),
                    device_id: options.device_id.clone(),
                    username: options.username.clone(),
                    address: None,
                    terminal: if options.scope == ConnectScope::Files {
                        ConnectionState::Closed
                    } else {
                        ConnectionState::Connecting
                    },
                    files: if options.scope == ConnectScope::Terminal {
                        ConnectionState::Closed
                    } else {
                        ConnectionState::Connecting
                    },
                    initial_directory: None,
                    attempt_finished: false,
                    authenticated: false,
                })
            });
            ssh.context.output.register(&id);
            let handle = ssh.clone();
            let _ = reply.send(Ok(id.clone()));
            jobs.spawn(async move {
                connect(id.clone(), options, handle.clone()).await;
                handle.context.session(&id, |s| s.attempt_finished = true);
            });
        }
    }
}
async fn connect(id: String, options: Connect, ssh: Ssh) {
    let first = client::connect(
        &ssh.context,
        &id,
        &options.addresses,
        &options.username,
        &options.password,
        options.trusted_key,
    )
    .await;
    let (connection, key, address) = match first {
        Ok(v) => v,
        Err(e) => {
            ssh.context.session(&id, |s| {
                if options.scope != ConnectScope::Files {
                    s.terminal = ConnectionState::Failed(e.clone());
                }
                if options.scope != ConnectScope::Terminal {
                    s.files = ConnectionState::Failed(e);
                }
            });
            return;
        }
    };
    ssh.context.session(&id, |s| {
        s.address = Some(address);
        s.authenticated = true;
    });
    if options.scope == ConnectScope::Files {
        if let Err(e) = ssh.files.attach(id.clone(), connection).await {
            ssh.context.file_failure(&id, e);
        }
        return;
    }
    if let Err(e) = ssh
        .terminal
        .attach(id.clone(), connection, options.size)
        .await
    {
        ssh.context
            .session(&id, |s| s.terminal = ConnectionState::Failed(e));
    }
    if options.scope == ConnectScope::Terminal {
        return;
    }
    // A second TCP + SSH authentication, not a second channel on the terminal's transport.
    let second = client::connect(
        &ssh.context,
        &id,
        &[address],
        &options.username,
        &options.password,
        Some(key),
    )
    .await;
    match second {
        Ok((connection, _, _)) => {
            if let Err(e) = ssh.files.attach(id.clone(), connection).await {
                ssh.context.file_failure(&id, e);
            }
        }
        Err(e) => ssh.context.file_failure(&id, e),
    }
    // options.password is dropped after both authentication attempts.
}
