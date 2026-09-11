mod commands;
mod replication;
use crate::{
    Event, Result, Status,
    app::{Command, Reply, Request},
    history,
    link::{self, Generation, LinkEvent, Outbound},
    model::Peer,
    ports::{ClipboardPort, Store},
    sync::Order,
};
use nooboard_network::{Identity, Message};
use tokio::{
    sync::{broadcast, mpsc, watch},
    task::JoinHandle,
};

pub(crate) struct Runtime {
    store: Store,
    identity: Identity,
    clipboard: Box<dyn ClipboardPort>,
    peer: Option<Peer>,
    state: Status,
    status: watch::Sender<Status>,
    events: broadcast::Sender<Event>,
    link: Option<JoinHandle<()>>,
    instance: u64,
    connection: Option<(Generation, mpsc::Sender<Outbound>)>,
    link_events: mpsc::Sender<LinkEvent>,
    incoming: mpsc::Receiver<LinkEvent>,
    order: Order,
    epoch: u64,
    peer_epoch: Option<u64>,
    observed_revision: u64,
}
impl Runtime {
    pub fn new(
        store: Store,
        identity: Identity,
        clipboard: Box<dyn ClipboardPort>,
        peer: Option<Peer>,
        state: Status,
        status: watch::Sender<Status>,
        events: broadcast::Sender<Event>,
    ) -> Self {
        let observed_revision = clipboard
            .subscribe()
            .borrow()
            .as_ref()
            .map(|s| s.revision)
            .unwrap_or(0);
        let (link_events, incoming) = mpsc::channel(64);
        Self {
            store,
            identity,
            clipboard,
            peer,
            state,
            status,
            events,
            link: None,
            instance: 0,
            connection: None,
            link_events,
            incoming,
            order: Order::default(),
            epoch: 0,
            peer_epoch: None,
            observed_revision,
        }
    }
    pub async fn run(mut self, mut requests: mpsc::Receiver<Request>) -> Result<()> {
        self.restart_link()?;
        let mut clipboard = self.clipboard.subscribe();
        // Catch a copy made after construction but before the task first ran.
        clipboard.mark_changed();
        let mut maintenance = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            let outcome = tokio::select! {
                request = requests.recv() => {
                    let Some(request) = request else { break };
                    if matches!(request.command, Command::Stop) {
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
                    match snapshot {
                        Ok(snapshot) => self.observe(snapshot, true).await,
                        Err(error) => Err(error.into()),
                    }
                }
                Some(event) = self.incoming.recv() => self.link_event(event).await,
                _ = maintenance.tick() => history::prune(&self.store, &self.state.settings).await,
            };
            if let Err(error) = outcome {
                self.emit(Event::Fault(error.to_string()));
            }
        }
        if let Some(task) = self.link.take() {
            task.abort();
            let _ = task.await;
        }
        self.connection = None;
        // Dropping the native clipboard owner joins its platform worker.
        Ok(())
    }
    fn emit(&self, event: Event) {
        let _ = self.events.send(event);
    }
    fn publish(&self) {
        self.status.send_replace(self.state.clone());
        self.emit(Event::Status(self.state.clone()));
    }
    fn restart_link(&mut self) -> Result<()> {
        if let Some(task) = self.link.take() {
            task.abort();
        }
        self.instance = self
            .instance
            .checked_add(1)
            .ok_or(crate::Error::Configuration)?;
        self.connection = None;
        self.peer_epoch = None;
        self.state.online = false;
        self.state.peer_accepting = false;
        if let Some(peer) = &self.peer {
            let tls = nooboard_network::TlsConfig::new(&self.identity, &peer.certificate)?;
            self.link = Some(link::start(
                self.instance,
                peer.endpoint.clone(),
                tls,
                self.link_events.clone(),
            ));
        }
        self.publish();
        Ok(())
    }
    async fn send(&mut self, message: Message) -> Result<()> {
        let sender = &self.connection.as_ref().ok_or(crate::Error::Offline)?.1;
        let result = link::send(sender, message).await;
        if result.is_err() {
            self.connection = None;
            self.state.online = false;
            self.state.peer_accepting = false;
            self.publish();
        }
        result
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        self.state.online = false;
        self.state.peer_accepting = false;
        self.publish();
        if let Some(task) = &self.link {
            task.abort();
        }
    }
}
