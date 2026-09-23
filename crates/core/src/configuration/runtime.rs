use super::{
    document,
    model::{Configuration, Peer},
};
use crate::{
    Error, PeerSettings, Result, Settings,
    runtime::message::{Reply, request},
};
use nooboard_network::TrustedPeer;
use nooboard_storage::Storage;
use tokio::sync::{mpsc, watch};
pub(crate) enum Change {
    Settings(Settings),
    SavePeer(TrustedPeer),
    RemovePeer(String),
    ConfigurePeer(String, PeerSettings),
    PeerNames(Vec<(String, String)>),
    Targets(Vec<String>),
}
pub(crate) struct Request {
    pub change: Change,
    pub reply: Reply<()>,
}
#[derive(Clone)]
pub(crate) struct Handle {
    pub requests: mpsc::Sender<Request>,
    pub state: watch::Receiver<Configuration>,
    pub stopped: watch::Receiver<bool>,
}
impl Handle {
    pub async fn change(&self, change: Change) -> Result<()> {
        request(&self.requests, &self.stopped, |reply| Request {
            change,
            reply,
        })
        .await
    }
    pub fn current(&self) -> Configuration {
        self.state.borrow().clone()
    }
}
pub(crate) fn channel(config: Configuration, stopped: watch::Receiver<bool>) -> (Handle, Runtime) {
    let (requests, inbox) = mpsc::channel(32);
    let (state, receiver) = watch::channel(config);
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
    state: watch::Sender<Configuration>,
}
impl Runtime {
    pub async fn run(mut self, storage: Storage, mut stop: watch::Receiver<bool>) -> Result<()> {
        loop {
            let request = tokio::select! {biased; _=stop.changed()=>break, request=self.inbox.recv()=>match request {Some(r)=>r,None=>break}};
            if request.reply.is_closed() {
                continue;
            }
            let result = self.apply(request.change, &storage).await;
            let _ = request.reply.send(result);
        }
        self.inbox.close();
        // During ordered shutdown producers have stopped; finish accepted persistence work.
        while let Some(request) = self.inbox.recv().await {
            let result = self.apply(request.change, &storage).await;
            let _ = request.reply.send(result);
        }
        Ok(())
    }
    async fn apply(&mut self, change: Change, storage: &Storage) -> Result<()> {
        let mut next = self.state.borrow().clone();
        match change {
            Change::Settings(settings) => next.settings = settings,
            Change::SavePeer(trusted) => {
                let id = trusted.identity.id.clone();
                let settings = if let Some(old) = next.peers.get(&id) {
                    if old.trusted.identity.certificate != trusted.identity.certificate {
                        return Err(Error::AlreadyPaired);
                    }
                    old.settings.clone()
                } else {
                    PeerSettings {
                        address: None,
                        auto_send: false,
                    }
                };
                next.peers.insert(id, Peer { trusted, settings });
            }
            Change::RemovePeer(id) => {
                if next.peers.remove(&id).is_none() {
                    return Err(Error::NotFound);
                }
                next.manual_targets.retain(|v| v != &id);
            }
            Change::ConfigurePeer(id, settings) => {
                next.peers.get_mut(&id).ok_or(Error::NotFound)?.settings = settings;
            }
            Change::PeerNames(names) => {
                let mut changed = false;
                for (id, name) in names {
                    if !super::model::valid_name(&name) {
                        return Err(Error::Configuration);
                    }
                    // A delayed status update cannot recreate an unpaired device.
                    if let Some(peer) = next.peers.get_mut(&id)
                        && peer.trusted.identity.device_name != name
                    {
                        peer.trusted.identity.device_name = name;
                        changed = true;
                    }
                }
                if !changed {
                    return Ok(());
                }
            }
            Change::Targets(targets) => next.manual_targets = targets,
        }
        next.validate()?;
        document::save(storage, &next).await?;
        next.revision += 1;
        next.automatic_revisions
            .retain(|id, _| next.peers.contains_key(id));
        for id in next.peers.keys() {
            if next.auto_send_enabled(id) && !self.state.borrow().auto_send_enabled(id) {
                next.automatic_revisions.insert(id.clone(), next.revision);
            }
        }
        self.state.send_replace(next);
        Ok(())
    }
}
