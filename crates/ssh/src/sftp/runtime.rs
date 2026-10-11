use super::client::Client;
use crate::{
    connection::client::Connection,
    runtime::{Context, Reply, request},
    *,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::{
    sync::mpsc,
    task::JoinSet,
    time::{Duration, interval},
};

enum Request {
    Attach {
        id: String,
        connection: Connection,
        reply: Reply<()>,
    },
    Close {
        id: String,
        reply: Reply<()>,
    },
}
#[derive(Clone)]
pub(crate) struct Handle {
    sender: mpsc::Sender<Request>,
    clients: Arc<Mutex<HashMap<String, Arc<Client>>>>,
    context: Context,
}
impl Handle {
    pub async fn attach(&self, id: String, connection: Connection) -> Result<()> {
        request(&self.sender, &self.context.stop, |reply| Request::Attach {
            id,
            connection,
            reply,
        })
        .await
    }
    pub fn get(&self, id: &str) -> Result<Arc<Client>> {
        let clients = self.clients.lock().map_err(|_| Error::Internal)?;
        let client = clients.get(id).ok_or(Error::SftpUnavailable)?;
        if client.is_closed() {
            return Err(Error::Connection("file transport disconnected".into()));
        }
        Ok(client.clone())
    }
    pub async fn close(&self, id: String) -> Result<()> {
        request(&self.sender, &self.context.stop, |reply| Request::Close {
            id,
            reply,
        })
        .await
    }
}
struct Connections {
    inbox: mpsc::Receiver<Request>,
    handle: Handle,
}
pub(crate) struct Runtime {
    connections: Connections,
    directory: super::directory::runtime::Runtime,
    transfer: super::transfer::runtime::Runtime,
}
pub(crate) fn create(
    context: Context,
) -> (
    Handle,
    super::directory::runtime::Handle,
    super::transfer::runtime::Handle,
    Runtime,
) {
    let (sender, inbox) = mpsc::channel(32);
    let handle = Handle {
        sender,
        clients: Arc::default(),
        context,
    };
    let (directory, directory_runtime) =
        super::directory::runtime::create(handle.context.clone(), handle.clone());
    let (transfer, transfer_runtime) =
        super::transfer::runtime::create(handle.context.clone(), handle.clone());
    (
        handle.clone(),
        directory,
        transfer,
        Runtime {
            connections: Connections { inbox, handle },
            directory: directory_runtime,
            transfer: transfer_runtime,
        },
    )
}
impl Runtime {
    pub async fn run(self) -> Result<()> {
        let mut jobs = JoinSet::new();
        jobs.spawn(self.connections.run());
        jobs.spawn(self.directory.run());
        jobs.spawn(self.transfer.run());
        while let Some(result) = jobs.join_next().await {
            result.map_err(|_| Error::Internal)??;
        }
        Ok(())
    }
}
impl Connections {
    pub async fn run(mut self) -> Result<()> {
        let mut stop = self.handle.context.stop.clone();
        let mut jobs = JoinSet::new();
        let mut check = interval(Duration::from_secs(1));
        loop {
            tokio::select! { biased;
                _ = stop.changed() => break,
                result = jobs.join_next(), if !jobs.is_empty() => { result.ok_or(Error::Internal)?.map_err(|_| Error::Internal)?; },
                _ = check.tick() => {
                    let clients = self.handle.clients.lock().map_err(|_| Error::Internal)?;
                    for (id, client) in clients.iter() {
                        if client.is_closed() { self.handle.context.session(id, |s| s.files = ConnectionState::Failed(Error::Connection("file transport disconnected".into()))); }
                    }
                },
                Some(message) = self.inbox.recv() => match message {
                    Request::Attach { id, connection, reply } => {
                        let handle = self.handle.clone();
                        jobs.spawn(async move {
                            let result = Client::open(connection, &handle.context.options).await;
                            match result {
                                Ok(client) => {
                                    let closed = handle.context.state.borrow().sessions.iter().any(|s| s.id == id && s.files == ConnectionState::Closed);
                                    if *handle.context.stop.borrow() || closed { client.shutdown().await; let _ = reply.send(Err(Error::Cancelled)); return; }
                                    let initial = client.initial.clone();
                                    handle.clients.lock().expect("file registry poisoned").insert(id.clone(), Arc::new(client));
                                    handle.context.session(&id, |s| { s.initial_directory = Some(initial); s.files = ConnectionState::Ready; });
                                    let _ = reply.send(Ok(()));
                                },
                                Err(e) => { let _ = reply.send(Err(e)); },
                            }
                        });
                    },
                    Request::Close { id, reply } => {
                        let result = {
                            let mut clients = self.handle.clients.lock().map_err(|_| Error::Internal)?;
                            if clients.get(&id).is_some_and(|c| Arc::strong_count(c) > 1) { Err(Error::Busy) }
                            else { Ok(clients.remove(&id)) }
                        };
                        match result {
                            Ok(client) => {
                                self.handle.context.session(&id, |s| s.files = ConnectionState::Closed);
                                jobs.spawn(async move { if let Some(client) = client { client.shutdown().await; } let _ = reply.send(Ok(())); });
                            },
                            Err(e) => { let _ = reply.send(Err(e)); },
                        }
                    },
                }
            }
        }
        while let Some(result) = jobs.join_next().await {
            result.map_err(|_| Error::Internal)?;
        }
        let clients =
            std::mem::take(&mut *self.handle.clients.lock().map_err(|_| Error::Internal)?);
        for (id, client) in clients {
            // Keep transport available while directory/transfer workers perform
            // their bounded CLOSE and temporary-file cleanup during shutdown.
            let deadline =
                tokio::time::Instant::now() + self.handle.context.options.request_timeout * 4;
            while Arc::strong_count(&client) > 1 && tokio::time::Instant::now() < deadline {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            client.shutdown().await;
            self.handle
                .context
                .session(&id, |s| s.files = ConnectionState::Closed);
        }
        Ok(())
    }
}
