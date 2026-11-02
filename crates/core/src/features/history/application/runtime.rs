use super::state::HistoryState;
use crate::contracts::storage::{
    HistoryId, HistoryQuery, HistorySource, NewHistoryEntry, RecordHistory, SourceFilter,
};
use crate::features::history::domain::policy;
use crate::{
    Error, HistoryEntry, Result,
    configuration::ports::settings::ConfigurationPort,
    message::{Reply, request},
    publish::Publish,
};
use tokio::sync::{mpsc, watch};
pub(crate) enum Request {
    Record {
        entry: NewHistoryEntry,
        retention: crate::contracts::storage::Retention,
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
        let now = crate::clock::now_ms();
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
        Runtime {
            inbox,
            state,
            revision: 0,
            error: None,
        },
    )
}
pub(crate) struct Runtime {
    inbox: mpsc::Receiver<Request>,
    state: watch::Sender<HistoryState>,
    /// Advances whenever stored history changes, so queries can be invalidated.
    revision: u64,
    error: Option<String>,
}
impl Runtime {
    pub async fn run(
        mut self,
        storage: std::sync::Arc<dyn crate::features::history::ports::storage::HistoryStore>,
        configuration: std::sync::Arc<dyn ConfigurationPort>,
        mut stop: watch::Receiver<bool>,
    ) -> Result<()> {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(30));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {biased;
             _=stop.changed()=>break,
             value=self.inbox.recv()=>match value{Some(value)=>self.handle(value,storage.as_ref()).await,None=>break},
             _=tick.tick()=>{match storage.prune(policy::retention(&configuration.current().settings,crate::clock::now_ms())).await{Ok(n)=>{if n>0{self.changed();}},Err(e)=>self.fault(e.to_string())}}
            }
            self.publish();
        }
        self.inbox.close();
        while let Some(value) = self.inbox.recv().await {
            self.handle(value, storage.as_ref()).await;
            self.publish();
        }
        Ok(())
    }
    fn changed(&mut self) {
        self.revision += 1;
    }
    fn fault(&mut self, error: String) {
        self.error = Some(error);
    }
    /// The only projection of this runtime's state to observers.
    fn publish(&self) {
        self.state.publish(HistoryState {
            revision: self.revision,
            error: self.error.clone(),
        });
    }
    /// A request's effects are published before its reply, so callers observe them on return.
    fn reply<T>(&mut self, reply: Reply<T>, result: Result<T>) {
        self.publish();
        let _ = reply.send(result);
    }
    async fn handle(
        &mut self,
        value: Request,
        storage: &dyn crate::features::history::ports::storage::HistoryStore,
    ) {
        match value {
            Request::Record { entry, retention } => {
                match storage.record(RecordHistory { entry, retention }).await {
                    Ok(_) => self.changed(),
                    Err(e) => self.fault(e.to_string()),
                }
            }
            Request::Query {
                contains,
                local,
                limit,
                offset,
                reply,
            } => {
                let result = storage
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
                    .map(|p| p.entries.into_iter().map(Into::into).collect());
                self.reply(reply, result);
            }
            Request::Get { id, reply } => {
                let result = async {
                    Ok(storage
                        .get(HistoryId::try_from(id)?)
                        .await?
                        .ok_or(Error::NotFound)?
                        .into())
                }
                .await;
                self.reply(reply, result);
            }
            Request::Delete { id, reply } => {
                let result = async {
                    if !storage.delete(HistoryId::try_from(id)?).await? {
                        return Err(Error::NotFound);
                    }
                    self.changed();
                    Ok(())
                }
                .await;
                self.reply(reply, result);
            }
            Request::Clear { reply } => {
                let result = storage.clear().await.map(|_| self.changed());
                self.reply(reply, result);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nooboard_storage::{BackendConfig, SqliteOptions, StorageService};
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn only_changes_to_stored_history_are_published_and_before_the_reply() {
        let (owner, storage) = StorageService::start(
            BackendConfig::Sqlite(SqliteOptions::in_memory()),
            Default::default(),
        )
        .await
        .unwrap();
        let store = crate::features::history::adapters::storage::port(storage);
        let (_stop, stopped) = watch::channel(false);
        let (handle, mut runtime) = channel(stopped);
        let mut state = handle.state.clone();
        let now = crate::clock::now_ms();
        runtime
            .handle(
                Request::Record {
                    entry: NewHistoryEntry {
                        text: "kept".into(),
                        source: HistorySource::Local,
                        copied_at_ms: now,
                    },
                    retention: policy::retention(&crate::Settings::default(), now),
                },
                store.as_ref(),
            )
            .await;
        runtime.publish();
        assert_eq!(state.borrow_and_update().revision, 1);
        // Reading changes nothing.
        let (reply, result) = oneshot::channel();
        runtime
            .handle(
                Request::Query {
                    contains: String::new(),
                    local: None,
                    limit: 10,
                    offset: 0,
                    reply,
                },
                store.as_ref(),
            )
            .await;
        let entries = result.await.unwrap().unwrap();
        assert!(!state.has_changed().unwrap(), "a query republished history");
        // A deletion is already published when its reply arrives.
        let (reply, result) = oneshot::channel();
        runtime
            .handle(
                Request::Delete {
                    id: entries[0].id,
                    reply,
                },
                store.as_ref(),
            )
            .await;
        result.await.unwrap().unwrap();
        assert!(
            state.has_changed().unwrap(),
            "deletion replied before publishing"
        );
        assert_eq!(state.borrow_and_update().revision, 2);
        owner.shutdown().await.unwrap();
    }
}
