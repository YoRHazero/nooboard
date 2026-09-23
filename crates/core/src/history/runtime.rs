use super::{model::HistoryState, policy};
use crate::{
    Error, HistoryEntry, Result,
    configuration::runtime::Handle as Configuration,
    runtime::message::{Reply, request},
};
use nooboard_storage::{
    HistoryId, HistoryQuery, HistorySource, NewHistoryEntry, RecordHistory, SourceFilter, Storage,
};
use tokio::sync::{mpsc, watch};
pub(crate) enum Request {
    Record {
        entry: NewHistoryEntry,
        retention: nooboard_storage::Retention,
    },
    Query {
        contains: String,
        local: Option<bool>,
        limit: u32,
        offset: u32,
        reply: Reply<Vec<HistoryEntry>>,
    },
    Get {
        id: i64,
        reply: Reply<HistoryEntry>,
    },
    Delete {
        id: i64,
        reply: Reply<()>,
    },
    Clear {
        reply: Reply<()>,
    },
}
#[derive(Clone)]
pub(crate) struct Handle {
    pub requests: mpsc::Sender<Request>,
    pub state: watch::Receiver<HistoryState>,
    pub stopped: watch::Receiver<bool>,
}
impl Handle {
    pub fn record(
        &self,
        text: String,
        peer: Option<String>,
        settings: &crate::Settings,
    ) -> Result<()> {
        if !settings.history {
            return Ok(());
        }
        let now = policy::now_ms();
        self.requests
            .try_send(Request::Record {
                entry: NewHistoryEntry {
                    text,
                    source: peer.map_or(HistorySource::Local, HistorySource::Remote),
                    copied_at_ms: now,
                },
                retention: policy::retention(settings, now),
            })
            .map_err(|_| Error::Busy)
    }
    pub async fn get(&self, id: i64) -> Result<HistoryEntry> {
        request(&self.requests, &self.stopped, |reply| Request::Get {
            id,
            reply,
        })
        .await
    }
}
pub(crate) fn channel(stopped: watch::Receiver<bool>) -> (Handle, Runtime) {
    let (requests, inbox) = mpsc::channel(64);
    let (state, receiver) = watch::channel(HistoryState::default());
    (
        Handle {
            requests,
            state: receiver,
            stopped,
        },
        Runtime { inbox, state },
    )
}
pub(crate) struct Runtime {
    inbox: mpsc::Receiver<Request>,
    state: watch::Sender<HistoryState>,
}
impl Runtime {
    pub async fn run(
        mut self,
        storage: Storage,
        configuration: Configuration,
        mut stop: watch::Receiver<bool>,
    ) -> Result<()> {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(30));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {biased;
             _=stop.changed()=>break,
             value=self.inbox.recv()=>match value{Some(value)=>self.handle(value,&storage).await,None=>break},
             _=tick.tick()=>{match storage.history().prune(policy::retention(&configuration.current().settings,policy::now_ms())).await{Ok(n)=>{if n>0{self.changed();}},Err(e)=>self.fault(e.to_string())}}
            }
        }
        self.inbox.close();
        while let Some(value) = self.inbox.recv().await {
            self.handle(value, &storage).await;
        }
        Ok(())
    }
    fn changed(&self) {
        self.state.send_modify(|s| s.revision += 1);
    }
    fn fault(&self, error: String) {
        self.state.send_modify(|s| s.error = Some(error));
    }
    async fn handle(&mut self, value: Request, storage: &Storage) {
        match value {
            Request::Record { entry, retention } => match storage
                .history()
                .record(RecordHistory { entry, retention })
                .await
            {
                Ok(_) => self.changed(),
                Err(e) => self.fault(e.to_string()),
            },
            Request::Query {
                contains,
                local,
                limit,
                offset,
                reply,
            } => {
                let result = storage
                    .history()
                    .query(HistoryQuery {
                        contains,
                        source: match local {
                            None => SourceFilter::All,
                            Some(true) => SourceFilter::Local,
                            Some(false) => SourceFilter::Remote,
                        },
                        limit,
                        offset: u64::from(offset),
                    })
                    .await
                    .map(|p| p.entries.into_iter().map(Into::into).collect())
                    .map_err(Into::into);
                let _ = reply.send(result);
            }
            Request::Get { id, reply } => {
                let result = async {
                    Ok(storage
                        .history()
                        .get(HistoryId::try_from(id)?)
                        .await?
                        .ok_or(Error::NotFound)?
                        .into())
                }
                .await;
                let _ = reply.send(result);
            }
            Request::Delete { id, reply } => {
                let result = async {
                    if !storage.history().delete(HistoryId::try_from(id)?).await? {
                        return Err(Error::NotFound);
                    }
                    self.changed();
                    Ok(())
                }
                .await;
                let _ = reply.send(result);
            }
            Request::Clear { reply } => {
                let result = storage
                    .history()
                    .clear()
                    .await
                    .map(|_| self.changed())
                    .map_err(Into::into);
                let _ = reply.send(result);
            }
        }
    }
}
