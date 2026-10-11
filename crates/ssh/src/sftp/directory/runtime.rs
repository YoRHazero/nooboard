use super::{model::DirectoryBatch, reader::Reader};
use crate::{
    runtime::{self, Context, Reply, request},
    sftp::{path, runtime::Handle as Files},
    *,
};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::{
    sync::{mpsc, watch},
    task::JoinSet,
    time::timeout,
};

enum Request {
    Validate {
        session: String,
        base: RemotePath,
        input: String,
        reply: Reply<RemotePath>,
    },
    Open {
        session: String,
        path: RemotePath,
        reply: Reply<DirectoryBatch>,
    },
    Next {
        cursor: String,
        reply: Reply<DirectoryBatch>,
    },
    Cancel {
        query: String,
        reply: Reply<()>,
    },
}
#[derive(Clone)]
pub(crate) struct Handle {
    sender: mpsc::Sender<Request>,
    context: Context,
}
impl Handle {
    pub async fn validate(
        &self,
        session: String,
        base: RemotePath,
        input: String,
    ) -> Result<RemotePath> {
        request(&self.sender, &self.context.stop, |reply| {
            Request::Validate {
                session,
                base,
                input,
                reply,
            }
        })
        .await
    }
    pub async fn open(&self, session: String, path: RemotePath) -> Result<DirectoryBatch> {
        request(&self.sender, &self.context.stop, |reply| Request::Open {
            session,
            path,
            reply,
        })
        .await
    }
    pub async fn next(&self, cursor: String) -> Result<DirectoryBatch> {
        request(&self.sender, &self.context.stop, |reply| Request::Next {
            cursor,
            reply,
        })
        .await
    }
    pub async fn cancel(&self, query: String) -> Result<()> {
        request(&self.sender, &self.context.stop, |reply| Request::Cancel {
            query,
            reply,
        })
        .await
    }
}
struct Query {
    sender: mpsc::Sender<(String, Reply<DirectoryBatch>)>,
    cancel: watch::Sender<bool>,
    busy: Arc<AtomicBool>,
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
impl Runtime {
    pub async fn run(mut self) -> Result<()> {
        let mut stop = self.context.stop.clone();
        let mut queries: HashMap<String, Query> = HashMap::new();
        let mut jobs = JoinSet::new();
        loop {
            tokio::select! { biased;
                _ = stop.changed() => break,
                result = jobs.join_next(), if !jobs.is_empty() => {
                    if let Some(id) = result.ok_or(Error::Internal)?.map_err(|_| Error::Internal)? { queries.remove(&id); }
                },
                Some(message) = self.inbox.recv() => match message {
                    Request::Validate { session, base, input, reply } => {
                        if jobs.len() >= self.context.options.max_directory_queries { let _ = reply.send(Err(Error::Limit)); continue; }
                        let client = self.files.get(&session);
                        jobs.spawn(async move {
                            let result = match client { Ok(client) => path::validate(&client, &base, &input).await, Err(e) => Err(e) };
                            let _ = reply.send(result); None
                        });
                    },
                    Request::Open { session, path, reply } => {
                        if jobs.len() >= self.context.options.max_directory_queries { let _ = reply.send(Err(Error::Limit)); continue; }
                        let client = match self.files.get(&session) { Ok(c) => c, Err(e) => { let _ = reply.send(Err(e)); continue; } };
                        let id = runtime::id();
                        let (sender, inbox) = mpsc::channel(1);
                        let (cancel, cancelled) = watch::channel(false);
                        let busy = Arc::new(AtomicBool::new(true));
                        queries.insert(id.clone(), Query { sender, cancel, busy: busy.clone() });
                        let context = self.context.clone();
                        jobs.spawn(async move {
                            match Reader::open(client, path.clone()).await {
                                Ok(reader) => QueryWorker { context, id: id.clone(), session, path, inbox, cancelled, busy }.run(reader, reply).await,
                                Err(e) => { let _ = reply.send(Err(e)); },
                            }
                            Some(id)
                        });
                    },
                    Request::Next { cursor, reply } => {
                        let query_id = cursor.split_once('/').map(|p| p.0).unwrap_or("");
                        let Some(query) = queries.get(query_id) else { let _ = reply.send(Err(Error::StaleCursor)); continue; };
                        if query.busy.swap(true, Ordering::SeqCst) { let _ = reply.send(Err(Error::Busy)); continue; }
                        if let Err(e) = query.sender.try_send((cursor, reply)) { query.busy.store(false, Ordering::SeqCst); let (_, reply) = e.into_inner(); let _ = reply.send(Err(Error::StaleCursor)); }
                    },
                    Request::Cancel { query, reply } => {
                        if let Some(query) = queries.remove(&query) { query.cancel.send_replace(true); }
                        let _ = reply.send(Ok(()));
                    },
                }
            }
        }
        for query in queries.values() {
            query.cancel.send_replace(true);
        }
        while let Some(result) = jobs.join_next().await {
            result.map_err(|_| Error::Internal)?;
        }
        Ok(())
    }
}
struct QueryWorker {
    context: Context,
    id: String,
    session: String,
    path: RemotePath,
    inbox: mpsc::Receiver<(String, Reply<DirectoryBatch>)>,
    cancelled: watch::Receiver<bool>,
    busy: Arc<AtomicBool>,
}
impl QueryWorker {
    async fn run(mut self, mut reader: Reader, first: Reply<DirectoryBatch>) {
        let mut reply = first;
        let mut stop = self.context.stop.clone();
        loop {
            if *stop.borrow() || *self.cancelled.borrow() || reply.is_closed() {
                break;
            }
            // Finish the bounded protocol request before closing its handle. Dropping
            // READDIR halfway through would silently skip an unknown server batch.
            let result = reader
                .next(self.context.options.directory_batch_size, || {
                    *stop.borrow() || *self.cancelled.borrow()
                })
                .await;
            if *stop.borrow() || *self.cancelled.borrow() {
                let _ = reply.send(Err(Error::Cancelled));
                break;
            }
            let (entries, complete) = match result {
                Ok(v) => v,
                Err(e) => {
                    let _ = reply.send(Err(e));
                    break;
                }
            };
            let cursor = (!complete).then(|| format!("{}/{}", self.id, runtime::id()));
            let batch = DirectoryBatch {
                session_id: self.session.clone(),
                query_id: self.id.clone(),
                path: self.path.clone(),
                entries,
                cursor: cursor.clone(),
                complete,
            };
            // A caller awakened by the reply must be able to use the next cursor
            // immediately. Concurrent users of the old token are rejected below.
            self.busy.store(false, Ordering::SeqCst);
            if reply.send(Ok(batch)).is_err() || complete {
                break;
            }
            loop {
                let next = tokio::select! {
                    _ = stop.changed() => None,
                    _ = self.cancelled.changed() => None,
                    result = timeout(self.context.options.directory_idle_timeout, self.inbox.recv()) => result.ok().flatten(),
                };
                match next {
                    Some((token, next)) if Some(&token) == cursor.as_ref() => {
                        reply = next;
                        break;
                    }
                    Some((_, next)) => {
                        let _ = next.send(Err(Error::StaleCursor));
                        self.busy.store(false, Ordering::SeqCst);
                    }
                    None => {
                        self.close_inbox();
                        reader.close().await;
                        return;
                    }
                }
            }
        }
        self.close_inbox();
        reader.close().await;
    }
    fn close_inbox(&mut self) {
        self.inbox.close();
        while let Ok((_, reply)) = self.inbox.try_recv() {
            let _ = reply.send(Err(Error::StaleCursor));
        }
    }
}
