#[path = "apply.rs"]
mod apply;
#[path = "observe.rs"]
mod observe;
#[path = "receive.rs"]
mod receive;
#[path = "send.rs"]
mod send;
use crate::contracts::clipboard::Snapshot;
use crate::contracts::network::NetworkEvent;
use crate::{
    Error, MessageId, Result, configuration::ports::settings::ConfigurationPort,
    features::clipboard::application::state::SyncState,
    features::clipboard::ports::clipboard::ClipboardPort,
    features::clipboard::ports::history::HistoryPort, message::Reply,
};
use std::sync::Arc;
use tokio::sync::{mpsc, watch};
type PreviewResult = (u64, crate::Result<(Vec<u8>, u32, u32)>);
pub(crate) enum Request {
    Send {
        targets: Option<Vec<String>>,
        reply: Reply<MessageId>,
    },
    Files {
        paths: Vec<std::path::PathBuf>,
        reply: Reply<()>,
    },
    Cancel {
        key: String,
        reply: Reply<()>,
    },
    CopyReceived {
        key: String,
        reply: Reply<()>,
    },
    CopyHistory {
        id: i64,
        reply: Reply<()>,
    },
}
pub(crate) struct Channels {
    pub requests: mpsc::Sender<Request>,
    pub events: mpsc::Sender<NetworkEvent>,
    pub state: watch::Receiver<SyncState>,
}
struct AutomaticBaseline {
    enabled_revision: u64,
    clipboard_revision: u64,
}
pub(crate) struct Runtime {
    requests: mpsc::Receiver<Request>,
    events: mpsc::Receiver<NetworkEvent>,
    state: watch::Sender<SyncState>,
    clipboard: Arc<dyn ClipboardPort>,
    content: Arc<dyn crate::features::clipboard::ports::content::ContentPort>,
    network: std::sync::Arc<dyn crate::features::clipboard::ports::network::NetworkPort>,
    config: std::sync::Arc<dyn ConfigurationPort>,
    history: std::sync::Arc<dyn HistoryPort>,
    observed: u64,
    previews: tokio::task::JoinSet<PreviewResult>,
    activity_stages: std::collections::BTreeMap<String, crate::contracts::network::TransferStage>,
    accepting: bool,
    ready: std::collections::BTreeMap<String, AutomaticBaseline>,
    pending_preview: Option<(u64, crate::contracts::clipboard::ImageData)>,
}
pub(crate) fn create(
    clipboard: Arc<dyn ClipboardPort>,
    content: Arc<dyn crate::features::clipboard::ports::content::ContentPort>,
    network: std::sync::Arc<dyn crate::features::clipboard::ports::network::NetworkPort>,
    config: std::sync::Arc<dyn ConfigurationPort>,
    history: std::sync::Arc<dyn HistoryPort>,
    initial: Snapshot,
) -> (Channels, Runtime) {
    let (requests, inbox) = mpsc::channel(32);
    let (events, receiver) = mpsc::channel(16);
    let pending_preview = match &initial.content {
        crate::contracts::clipboard::ReadState::Ready(
            crate::contracts::clipboard::Payload::Image(image),
        ) => Some((initial.revision, image.clone())),
        _ => None,
    };
    let observed = initial.revision;
    let accepting = config.current().settings.accepting();
    let (state, snapshot) = watch::channel(SyncState {
        current: crate::CurrentClipboard::from_native(initial, None),
        operations: Default::default(),
        activities: Default::default(),
        fault: None,
        application_failures: Default::default(),
        sequence: 0,
        effective_revision: 0,
    });
    (
        Channels {
            requests,
            events,
            state: snapshot,
        },
        Runtime {
            requests: inbox,
            events: receiver,
            state,
            clipboard,
            content,
            network,
            config,
            history,
            observed,
            accepting,
            activity_stages: Default::default(),
            ready: Default::default(),
            previews: Default::default(),
            pending_preview,
        },
    )
}
impl Runtime {
    pub async fn run(mut self, mut stop: watch::Receiver<bool>) -> Result<()> {
        let mut changes = self.clipboard.subscribe();
        let mut health = self.clipboard.subscribe_status();
        let mut config = self.config.subscribe();
        let mut network = self.network.subscribe();
        self.start_preview();
        self.reconcile().await?;
        // Subscription may already contain a copy made between baseline read and startup.
        let initial = changes.borrow_and_update().clone();
        if let Some(initial) = initial {
            self.observe(initial, true).await?;
        }
        loop {
            if *stop.borrow() {
                break;
            }
            self.prune();
            self.content_activities();
            tokio::select! {
                _ = stop.changed() => break,
                // All native writes execute through this actor.
                Some(event) = self.events.recv() => {
                    if let Err(error) = self.receive(event).await { self.fault(&error); }
                },
                request = self.requests.recv() => match request {
                    Some(request) => self.command(request).await,
                    None => break,
                },
                changed = changes.changed() => {
                    if changed.is_err() { return Err(Error::Stopped); }
                    let snapshot = changes.borrow_and_update().clone();
                    if let Some(snapshot) = snapshot && let Err(error) = self.observe(snapshot, true).await { self.fault(&error); }
                },
                changed = network.changed() => {
                    if changed.is_err() { return Err(Error::Stopped); }
                    if let Err(error) = self.refresh_routes().await { self.fault(&error); }
                },
                changed = config.changed() => {
                    if changed.is_err() { return Err(Error::Stopped); }
                    if let Err(error) = self.reconcile().await { self.fault(&error); }
                },
                changed = async { health.as_mut().expect("guarded subscription").changed().await }, if health.is_some() => {
                    let status = health.as_ref().unwrap().borrow().clone();
                    if changed.is_err() { health = None; }
                    match status {
                        crate::contracts::clipboard::ServiceStatus::Stopped { .. } => return Err(Error::Stopped),
                        crate::contracts::clipboard::ServiceStatus::Unavailable(error) => self.fault(&error.into()),
                        _ => {},
                    }
                },
                result = self.previews.join_next(), if !self.previews.is_empty() => {
                    if let Some(Ok((revision, Ok((bytes, width, height))))) = result {
                        use base64::Engine;
                        self.state.send_if_modified(|state| {
                            if state.current.revision != revision { return false; }
                            state.current.preview = Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)));
                            state.current.image_width = Some(width);
                            state.current.image_height = Some(height);
                            true
                        });
                    }
                    self.start_preview();
                },
            }
        }
        self.network.set_accepting(false).await?;
        // Finish the current write before reaching this point; reject queued application work.
        self.events.close();
        while let Some(event) = self.events.recv().await {
            self.reject_pending(event).await;
        }
        self.requests.close();
        self.pending_preview = None;
        while self.previews.join_next().await.is_some() {}
        Ok(())
    }
    async fn refresh_routes(
        &mut self,
    ) -> Result<crate::configuration::domain::model::Configuration> {
        let config = self.config.current();
        let targets =
            crate::features::clipboard::domain::policy::targets(&config, &self.network.status());
        let enabled_revision =
            |id: &String| config.automatic_revisions.get(id).copied().unwrap_or(0);
        self.ready.retain(|id, baseline| {
            targets.contains(id) && baseline.enabled_revision == enabled_revision(id)
        });
        let added = targets
            .into_iter()
            .filter(|id| !self.ready.contains_key(id))
            .collect::<Vec<_>>();
        if !added.is_empty() {
            let baseline = self.clipboard.read().await?.revision;
            for id in added {
                let enabled_revision = enabled_revision(&id);
                self.ready.insert(
                    id,
                    AutomaticBaseline {
                        enabled_revision,
                        clipboard_revision: baseline,
                    },
                );
            }
        }
        Ok(config)
    }
    fn prune(&self) {
        let network = self.network.status();
        self.state.send_if_modified(|s| {
            let before = s.operations.len();
            let mut ended = 0;
            s.operations.retain(|op| {
                if network
                    .transfers
                    .iter()
                    .any(|row| op.matches(row) && !row.stage.is_terminal())
                {
                    true
                } else {
                    ended += 1;
                    ended <= 64
                }
            });
            s.application_failures.retain(|(id, peer)| {
                s.operations
                    .iter()
                    .any(|op| &op.id == id && op.incoming_peer.as_ref() == Some(peer))
            });
            s.operations.len() != before
        });
    }
    fn fault(&self, e: &Error) {
        self.state.send_modify(|s| s.fault = Some(e.to_string()));
    }
    async fn reconcile(&mut self) -> Result<()> {
        let config = self.refresh_routes().await?;
        self.network.set_paused(config.settings.paused).await?;
        if self.accepting != config.settings.accepting() {
            self.network
                .set_accepting(config.settings.accepting())
                .await?;
            self.accepting = config.settings.accepting();
        }
        if config.settings.paused || config.settings.mode == crate::Mode::Manual {
            let ids = self
                .state
                .borrow()
                .operations
                .iter()
                .filter(|op| config.settings.paused || op.automatic)
                .map(|op| op.id.clone())
                .collect::<Vec<_>>();
            for id in ids {
                let _ = self.network.cancel_transfer(id).await;
            }
        }
        // A per-device opt-out only cancels that device's automatic destinations.
        if !config.settings.paused && config.settings.mode == crate::Mode::Automatic {
            let rows = self.network.status().transfers;
            for row in rows
                .iter()
                .filter(|row| !row.incoming && !row.stage.is_terminal())
            {
                let automatic = self
                    .state
                    .borrow()
                    .operations
                    .iter()
                    .any(|op| op.matches(row) && op.automatic);
                if automatic
                    && !config
                        .peers
                        .get(&row.peer)
                        .is_some_and(|peer| peer.settings.auto_send)
                {
                    let _ = self
                        .network
                        .cancel_delivery(row.peer.clone(), row.id.clone())
                        .await;
                }
            }
        }
        self.state
            .send_modify(|s| s.effective_revision = config.revision);
        Ok(())
    }
    async fn command(&mut self, request: Request) {
        match request {
            Request::Send { targets, reply } => {
                let result = self.send_current(targets).await;
                let _ = reply.send(result);
            }
            Request::Files { paths, reply } => {
                let config = self.config.current();
                let result = self
                    .send_payload(
                        crate::contracts::clipboard::Payload::Files(paths),
                        config.manual_targets,
                        false,
                    )
                    .await
                    .map(|_| ());
                let _ = reply.send(result);
            }
            Request::Cancel { key, reply } => {
                let result = async {
                    let row = self
                        .network
                        .status()
                        .transfers
                        .into_iter()
                        .find(|row| {
                            crate::features::clipboard::domain::model::transfer_key(row) == key
                        })
                        .ok_or(Error::NotFound)?;
                    if row.incoming {
                        self.network.cancel_incoming(row.peer, row.id).await?;
                    } else {
                        self.network.cancel_delivery(row.peer, row.id).await?;
                    }
                    Ok(())
                }
                .await;
                let _ = reply.send(result);
            }
            Request::CopyReceived { key, reply } => {
                let result = async {
                    let row = self
                        .network
                        .status()
                        .transfers
                        .into_iter()
                        .find(|r| {
                            crate::features::clipboard::domain::model::transfer_key(r) == key
                                && !r.saved_paths.is_empty()
                        })
                        .ok_or(Error::NotFound)?;
                    let payload = crate::contracts::clipboard::Payload::Files(row.saved_paths);
                    self.apply(payload, None).await?;
                    Ok(())
                }
                .await;
                let _ = reply.send(result);
            }
            Request::CopyHistory { id, reply } => {
                let result = async {
                    let entry = self.history.get(id).await?;
                    self.apply(crate::contracts::clipboard::Payload::Text(entry.text), None)
                        .await?;
                    Ok(())
                }
                .await;
                let _ = reply.send(result);
            }
        }
    }
    fn activity(
        &self,
        kind: crate::ActivityKind,
        text: &str,
        source: Option<String>,
        id: Option<MessageId>,
        content: Option<(String, String, crate::ContentStage)>,
    ) {
        let config = self.config.current();
        let device_name = source
            .as_ref()
            .and_then(|id| config.peers.get(id))
            .map(|p| p.trusted.identity.device_name.clone());
        self.state.send_modify(|s| {
            s.sequence += 1;
            s.activities.push_front(crate::ActivityRecord {
                sequence: s.sequence,
                kind,
                summary: text.chars().take(160).collect(),
                at_ms: crate::clock::now_ms(),
                source,
                device_name,
                message_id: id,
                content_task: content.as_ref().map(|(key, _, _)| key.clone()),
                content_node: content.as_ref().map(|(_, node, _)| node.clone()),
                content_stage: content.map(|(_, _, stage)| stage),
            });
            s.activities.truncate(30);
        });
    }
    fn content_activities(&mut self) {
        let network = self.network.status();
        self.activity_stages.retain(|key, _| {
            network
                .transfers
                .iter()
                .any(|row| crate::features::clipboard::domain::model::transfer_key(row) == *key)
        });
        for row in &network.transfers {
            let operation = self
                .state
                .borrow()
                .operations
                .iter()
                .find(|op| op.matches(row) && op.kind.is_some())
                .cloned();
            let Some(operation) = operation else {
                continue;
            };
            let key = crate::features::clipboard::domain::model::transfer_key(row);
            let previous = self.activity_stages.insert(key.clone(), row.stage);
            if previous.is_none() || (row.stage.is_terminal() && previous != Some(row.stage)) {
                self.activity(
                    if row.incoming {
                        crate::ActivityKind::Received
                    } else {
                        crate::ActivityKind::Sent
                    },
                    &operation.names.join(", "),
                    Some(row.peer.clone()),
                    Some(row.id.clone()),
                    Some((
                        key,
                        if row.stage.is_terminal() {
                            "finished"
                        } else {
                            "started"
                        }
                        .into(),
                        crate::features::clipboard::domain::model::content_stage(row.stage),
                    )),
                );
            }
        }
    }
    fn start_preview(&mut self) {
        if self.previews.is_empty()
            && let Some((revision, image)) = self.pending_preview.take()
        {
            let content = self.content.clone();
            self.previews
                .spawn_blocking(move || (revision, content.thumbnail(&image)));
        }
    }
}
