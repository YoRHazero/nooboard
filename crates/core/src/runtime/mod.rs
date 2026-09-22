mod commands;
mod connections;
mod content;
mod local_network;
mod onboarding;
mod replication;
use crate::{
    Event, Result, Status,
    app::{Command, Reply, Request},
    devices::Configuration,
    history,
    link::{Dial, LinkEvent, Listener, Session},
    ports::{ClipboardPort, Store},
    sync::Received,
    transfers::Transfers,
};
use nooboard_network::Identity;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};
use tokio::sync::{broadcast, mpsc, watch};

struct PeerSession {
    session: Option<Session>,
    dial: Option<Dial>,
    online: watch::Sender<bool>,
    generation: u64,
    dial_generation: u64,
    preferred: bool,
    local_epoch: u64,
    peer_epoch: Option<u64>,
    accepting: bool,
}
impl Drop for Runtime {
    fn drop(&mut self) {
        if let Some(task) = &self.preview_task {
            task.abort();
        }
    }
}
impl PeerSession {
    fn new() -> Self {
        let (online, _) = watch::channel(false);
        Self {
            session: None,
            dial: None,
            online,
            generation: 0,
            dial_generation: 0,
            preferred: false,
            local_epoch: 0,
            peer_epoch: None,
            accepting: false,
        }
    }
}
pub(crate) struct Runtime {
    onboarding: crate::onboarding::Onboarding,
    store: Store,
    identity: Identity,
    clipboard: std::sync::Arc<dyn ClipboardPort>,
    content: crate::content_transfer::ContentTransfers,
    preview_task: Option<tokio::task::JoinHandle<()>>,
    config: Configuration,
    peers: BTreeMap<String, PeerSession>,
    listener: Listener,
    state: Status,
    status: watch::Sender<Status>,
    events: broadcast::Sender<Event>,
    link_events: mpsc::Sender<LinkEvent>,
    incoming: mpsc::Receiver<LinkEvent>,
    received: Received,
    transfers: Transfers,
    namespace: String,
    sequence: u64,
    generation: u64,
    observed_revision: u64,
    view: crate::view::ViewState,
    pub(crate) snapshots: watch::Sender<crate::AppSnapshot>,
}
impl Runtime {
    pub async fn new(
        store: Store,
        identity: Identity,
        clipboard: Box<dyn ClipboardPort>,
        config: Configuration,
        status: watch::Sender<Status>,
        events: broadcast::Sender<Event>,
    ) -> Result<Self> {
        let initial = clipboard.read().await?;
        let observed_revision = initial.revision;
        let (link_events, incoming) = mpsc::channel(512);
        let listener = Listener::bind(
            &config.settings.listen_address,
            config.tls(&identity)?,
            link_events.clone(),
        )
        .await?;
        let state = status.borrow().clone();
        let namespace = nooboard_network::new_session_id()?;
        let view = crate::view::ViewState::new(namespace.clone(), state.clone(), initial.clone());
        let (snapshots, _) = watch::channel(view.snapshot.clone());
        let (sender, events_pairing) = mpsc::channel(32);
        let endpoint = nooboard_network::pairing::Endpoint::bind(
            &config.settings.pairing_listen_address,
            nooboard_network::pairing::Contact {
                device_name: config.settings.device_name.clone(),
                certificate: identity.certificate().to_vec(),
                sync_port: listener
                    .address
                    .parse::<std::net::SocketAddr>()
                    .expect("bound address")
                    .port(),
            },
            sender.clone(),
        )
        .await?;
        let (_, nearby) = watch::channel(Vec::new());
        let onboarding = crate::onboarding::Onboarding {
            snapshot: crate::OnboardingSnapshot {
                pairing_address: endpoint.address.clone(),
                ..Default::default()
            },
            endpoint,
            discovery: None,
            discovery_refreshed: None,
            nearby,
            events: events_pairing,
            sender,
            control: None,
            expected: None,
        };
        let mut runtime = Self {
            onboarding,
            store,
            identity,
            clipboard: std::sync::Arc::from(clipboard),
            content: crate::content_transfer::ContentTransfers::default(),
            preview_task: None,
            config,
            peers: BTreeMap::new(),
            listener,
            state,
            status,
            events,
            link_events,
            incoming,
            received: Received::default(),
            transfers: Transfers::default(),
            namespace,
            view,
            snapshots,
            sequence: 0,
            generation: 0,
            observed_revision,
        };
        for peer in runtime.config.peers.keys().cloned().collect::<Vec<_>>() {
            runtime.start_dial(&peer)?;
        }
        runtime.refresh_local_network();
        runtime.preview_content(&initial);
        runtime.publish();
        Ok(runtime)
    }
    pub async fn run(mut self, mut requests: mpsc::Receiver<Request>) -> Result<()> {
        let mut clipboard = self.clipboard.subscribe();
        clipboard.mark_changed();
        let mut clipboard_status = self.clipboard.subscribe_status();
        let mut maintenance = tokio::time::interval(Duration::from_secs(30));
        let mut receipts = tokio::time::interval(Duration::from_secs(1));
        let mut interfaces = tokio::time::interval(Duration::from_secs(5));
        interfaces.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            let outcome = tokio::select! {
                request = requests.recv() => {
                    let Some(request) = request else { break };
                    if matches!(request.command, Command::Stop) {
                        for peer in self.peers.keys().cloned().collect::<Vec<_>>() { self.disconnect(&peer); }
                        self.peers.clear();
                        let _ = request.response.send(Ok(Reply::Done));
                        break;
                    }
                    let result = self.command(request.command).await;
                    let _ = request.response.send(result);
                    Ok(())
                }
                changed = clipboard.changed() => {
                    if changed.is_err() { return Err(crate::Error::Stopped); }
                    let snapshot = clipboard.borrow_and_update().clone();
                    match snapshot { Some(snapshot) => self.observe(snapshot, true).await, None => Ok(()) }
                }
                changed = async { clipboard_status.as_mut().expect("enabled status subscription").changed().await }, if clipboard_status.is_some() => {
                    let status = clipboard_status.as_mut().expect("status subscription").borrow_and_update().clone();
                    if changed.is_err() { clipboard_status = None; }
                    match status {
                        nooboard_clipboard::ServiceStatus::Unavailable(error) => Err(error.into()),
                        nooboard_clipboard::ServiceStatus::Stopped { error } => return Err(error.unwrap_or(nooboard_clipboard::Error::Stopped).into()),
                        _ => Ok(()),
                    }
                },
                Some(event) = self.incoming.recv() => self.link_event(event).await,
                Some(event) = self.content.incoming.recv() => { self.content_event(event).await; Ok(()) },
                Some(event) = self.onboarding.events.recv() => self.pairing_event(event).await,
                _ = interfaces.tick() => {
                    if self.refresh_local_network() { self.publish_snapshot(); }
                    Ok(())
                },
                Ok(()) = self.onboarding.nearby.changed(), if self.onboarding.discovery.is_some() => {
                    self.onboarding.snapshot.nearby=self.onboarding.nearby.borrow_and_update().clone();
                    for id in self.config.peers.keys().cloned().collect::<Vec<_>>() { self.start_dial(&id)?; }
                    self.publish_snapshot(); Ok(())
                },
                _ = maintenance.tick() => {
                    let result = history::prune(&self.store, &self.config.settings).await;
                    if matches!(result, Ok(true)) { self.emit(Event::HistoryChanged); }
                    result.map(|_| ())
                },
                _ = receipts.tick() => {
                    let expired = self.transfers.expire(Instant::now());
                    if !expired.is_empty() {
                        for transfer in expired { self.emit(Event::Transfer(transfer)); }
                        self.publish();
                    }
                    Ok(())
                }
            };
            if let Err(error) = outcome {
                self.fault(None, error.to_string());
            }
        }
        Ok(())
    }
    fn emit(&mut self, event: Event) {
        match &event {
            Event::HistoryChanged => self.view.snapshot.history_revision += 1,
            Event::Fault { peer, message } => {
                self.view.snapshot.fault = Some(crate::Fault {
                    sequence: self.view.snapshot.revision + 1,
                    peer: peer.clone(),
                    message: message.clone(),
                })
            }
            _ => {}
        }
        self.publish_snapshot();
        let _ = self.events.send(event);
    }
    fn publish_snapshot(&mut self) {
        self.content.prune();
        self.view.snapshot.content_transfers = self.content.snapshot();
        self.view.snapshot.onboarding = self.onboarding.snapshot.clone();
        self.snapshots.send_replace(
            self.view
                .publish(self.state.clone(), self.transfers.snapshot()),
        );
    }
    fn fault(&mut self, peer: Option<String>, message: String) {
        self.emit(Event::Fault { peer, message });
    }
    fn fresh_generation(&mut self) -> Result<u64> {
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or(crate::Error::Configuration)?;
        Ok(self.generation)
    }
    fn publish(&mut self) {
        self.state.settings = self.config.settings.clone();
        self.state.listen_address = self.listener.address.clone();
        self.state.manual_targets = self.config.manual_targets.clone();
        self.state.peers = self
            .config
            .peers
            .values()
            .map(|p| {
                let session = self.peers.get(&p.noob_id);
                crate::PeerStatus {
                    noob_id: p.noob_id.clone(),
                    device_name: p.device_name.clone(),
                    fingerprint: nooboard_network::fingerprint(&p.certificate),
                    settings: p.settings.clone(),
                    online: session.is_some_and(|s| s.session.is_some()),
                    accepting: session.is_some_and(|s| s.accepting),
                }
            })
            .collect();
        self.state.transfers = self.transfers.snapshot();
        self.status.send_replace(self.state.clone());
        self.emit(Event::Status(self.state.clone()));
    }
    fn delivery(
        &mut self,
        id: &nooboard_network::MessageId,
        peer: &str,
        state: crate::DeliveryState,
    ) {
        if let Some(transfer) = self.transfers.update(id, peer, state) {
            self.emit(Event::Transfer(transfer));
            self.publish();
        }
    }
}
