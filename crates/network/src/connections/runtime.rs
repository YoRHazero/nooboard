use super::{
    dial,
    model::ConnectionStatus,
    queue::Outbox,
    session,
    transport::tls_tcp::{Connection, TlsConfig},
};
use crate::{
    discovery::NearbyDevice,
    error::{Failure, InternalResult as Result},
    identity::{TrustedPeer, material::Identity},
    options::Options,
    pairing::PairingId,
    transfer::protocol::{Message, MessageId},
};
use std::{
    collections::{BTreeMap, HashSet},
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};
use tokio::{
    net::TcpListener,
    sync::{mpsc, oneshot, watch},
    task::JoinSet,
};

pub(crate) enum Request {
    Trust {
        peer: TrustedPeer,
        pairing: Option<PairingId>,
        reply: oneshot::Sender<Result<()>>,
    },
    FinishPairing {
        id: PairingId,
    },
    Revoke {
        peer: String,
        reply: oneshot::Sender<Result<()>>,
    },
    Pause {
        paused: bool,
        reply: oneshot::Sender<Result<()>>,
    },
    Enable {
        peer: String,
        enabled: bool,
        reply: oneshot::Sender<Result<()>>,
    },
}
pub(crate) struct Handle {
    pub requests: mpsc::Sender<Request>,
    pub status: watch::Receiver<Vec<ConnectionStatus>>,
    pub hints: watch::Sender<Vec<NearbyDevice>>,
    pub address: SocketAddr,
}
pub(crate) enum Event {
    Connected {
        peer: String,
        generation: u64,
        outbox: Outbox,
    },
    Offline {
        peer: String,
        generation: u64,
    },
    Message {
        peer: String,
        generation: u64,
        message: Message,
    },
    Started {
        peer: String,
        generation: u64,
        id: MessageId,
    },
    Written {
        peer: String,
        generation: u64,
        id: MessageId,
    },
}
pub(super) enum SessionEvent {
    Message {
        peer: String,
        generation: u64,
        message: Message,
    },
    Started {
        peer: String,
        generation: u64,
        id: MessageId,
    },
    Written {
        peer: String,
        generation: u64,
        id: MessageId,
    },
}
struct Slot {
    generation: u64,
    peer_address: SocketAddr,
    paused: bool,
    outbox: Outbox,
}
enum Completion {
    Handshake {
        expected: Option<String>,
        revision: u64,
        result: Result<Connection>,
    },
    Candidate {
        revision: u64,
        connection: Connection,
    },
    Session {
        peer: String,
        generation: u64,
        unpaired: bool,
    },
}
pub(crate) struct Runtime {
    identity: Arc<Identity>,
    own_id: String,
    name: String,
    listener: TcpListener,
    requests: mpsc::Receiver<Request>,
    status: watch::Sender<Vec<ConnectionStatus>>,
    events: mpsc::Sender<Event>,
    hints: watch::Receiver<Vec<NearbyDevice>>,
    stop: watch::Receiver<bool>,
    peers: BTreeMap<String, TrustedPeer>,
    disabled: HashSet<String>,
    pairing: BTreeMap<String, PairingId>,
    active: BTreeMap<String, Slot>,
    pending: HashSet<String>,
    next_address: BTreeMap<String, usize>,
    unpaired: BTreeMap<String, u64>,
    discovered: HashSet<String>,
    paused: bool,
    revision: u64,
    generation: u64,
    tls: TlsConfig,
}
impl Runtime {
    pub async fn bind(
        options: &Options,
        identity: Arc<Identity>,
        stop: watch::Receiver<bool>,
    ) -> Result<(Self, Handle, mpsc::Receiver<Event>)> {
        let own_id = identity.noob_id()?;
        let mut peers = BTreeMap::new();
        for peer in &options.trusted_peers {
            peer.validate(&own_id)?;
            if peers
                .insert(peer.identity.id.clone(), peer.clone())
                .is_some()
            {
                return Err(Failure::InvalidArgument("duplicate peer"));
            }
        }
        let tls = make_tls(&identity, &peers)?;
        let listener = TcpListener::bind(options.listen).await?;
        let address = listener.local_addr()?;
        let (tx, requests) = mpsc::channel(options.queue_capacity);
        let (status, rx) = watch::channel(vec![]);
        let (events, incoming) = mpsc::channel(64);
        let (hint_tx, hints) = watch::channel(vec![]);
        let runtime = Self {
            identity,
            own_id,
            name: options.device_name.clone(),
            listener,
            requests,
            status,
            events,
            hints,
            stop,
            peers,
            disabled: HashSet::new(),
            pairing: BTreeMap::new(),
            active: BTreeMap::new(),
            pending: HashSet::new(),
            next_address: BTreeMap::new(),
            unpaired: BTreeMap::new(),
            discovered: HashSet::new(),
            paused: false,
            revision: 0,
            generation: 0,
            tls,
        };
        runtime.publish();
        Ok((
            runtime,
            Handle {
                requests: tx,
                status: rx,
                hints: hint_tx,
                address,
            },
            incoming,
        ))
    }
    fn publish(&self) {
        self.status.send_replace(
            self.peers
                .iter()
                .map(|(id, peer)| ConnectionStatus {
                    peer: id.clone(),
                    device_name: peer.identity.device_name.clone(),
                    connected: self.active.contains_key(id),
                    peer_address: self.active.get(id).map(|s| s.peer_address),
                    accepting: false,
                    paused: self.active.get(id).is_some_and(|s| s.paused),
                    unpaired_notice: self.unpaired.get(id).copied(),
                    generation: self.active.get(id).map_or(0, |s| s.generation),
                })
                .collect(),
        );
    }
    fn disconnect(&mut self, peer: &str) {
        if let Some(slot) = self.active.remove(peer) {
            slot.outbox.close();
            let _ = self.events.try_send(Event::Offline {
                peer: peer.into(),
                generation: slot.generation,
            });
        }
    }
    fn request(&mut self, request: Request) {
        match request {
            Request::Trust {
                peer,
                pairing,
                reply,
            } => {
                if reply.is_closed() {
                    return;
                }
                let result = (|| {
                    peer.validate(&self.own_id)?;
                    let id = peer.identity.id.clone();
                    if self.peers.len() >= 64 && !self.peers.contains_key(&id) {
                        return Err(Failure::Busy);
                    }
                    let mut peers = self.peers.clone();
                    peers.insert(id.clone(), peer);
                    let tls = make_tls(&self.identity, &peers)?;
                    let changed_identity = self.peers.get(&id).is_some_and(|old| {
                        old.identity.certificate != peers[&id].identity.certificate
                    });
                    if changed_identity || pairing.is_some() {
                        self.disconnect(&id);
                    }
                    self.peers = peers;
                    self.tls = tls;
                    self.revision = self.revision.checked_add(1).ok_or(Failure::Internal)?;
                    self.disabled.remove(&id);
                    self.unpaired.remove(&id);
                    if let Some(pairing) = pairing {
                        self.pairing.insert(id, pairing);
                    }
                    Ok(())
                })();
                let _ = reply.send(result);
            }
            Request::FinishPairing { id } => {
                let previous = self.pairing.len();
                self.pairing.retain(|_, session| session != &id);
                if self.pairing.len() != previous {
                    self.revision = self.revision.wrapping_add(1);
                }
            }
            Request::Revoke { peer, reply } => {
                if reply.is_closed() {
                    return;
                }
                let mut peers = self.peers.clone();
                peers.remove(&peer);
                let result = make_tls(&self.identity, &peers).map(|tls| {
                    if let Some(slot) = self.active.remove(&peer) {
                        slot.outbox.revoke();
                        let _ = self.events.try_send(Event::Offline {
                            peer: peer.clone(),
                            generation: slot.generation,
                        });
                    }
                    self.disabled.remove(&peer);
                    self.next_address.remove(&peer);
                    self.unpaired.remove(&peer);
                    self.pairing.remove(&peer);
                    self.peers = peers;
                    self.tls = tls;
                    self.revision = self.revision.wrapping_add(1);
                });
                let _ = reply.send(result);
            }
            Request::Pause { paused, reply } => {
                if !reply.is_closed() && self.paused != paused {
                    self.paused = paused;
                    for slot in self.active.values() {
                        if slot.outbox.control(Message::Paused { paused }).is_err() {
                            slot.outbox.close();
                        }
                    }
                }
                let _ = reply.send(Ok(()));
            }
            Request::Enable {
                peer,
                enabled,
                reply,
            } => {
                if reply.is_closed() {
                    return;
                }
                let result = if !self.peers.contains_key(&peer) {
                    Err(Failure::NotFound)
                } else {
                    if enabled {
                        self.disabled.remove(&peer);
                    } else {
                        self.disabled.insert(peer.clone());
                        self.disconnect(&peer);
                    }
                    Ok(())
                };
                let _ = reply.send(result);
            }
        }
        self.publish();
    }
    fn mark_unpaired(&mut self, id: &str) -> Result<()> {
        if !self.peers.contains_key(id)
            || self.unpaired.contains_key(id)
            || self.pairing.contains_key(id)
        {
            return Ok(());
        }
        self.generation = self.generation.checked_add(1).ok_or(Failure::Internal)?;
        self.unpaired.insert(id.to_owned(), self.generation);
        self.disconnect(id);
        self.publish();
        Ok(())
    }
    fn refresh_discovery(&mut self, devices: &[NearbyDevice]) {
        let discovered: HashSet<_> = devices.iter().map(|h| h.noob_id.clone()).collect();
        // A new appearance starts a fresh status check. Receipt state is not persisted.
        for id in discovered.difference(&self.discovered) {
            self.unpaired.remove(id);
        }
        self.discovered = discovered;
        self.publish();
    }
    fn dial(&mut self, tasks: &mut JoinSet<Completion>) {
        for (id, peer) in &self.peers {
            if self.disabled.contains(id)
                || self.pairing.contains_key(id)
                || self.unpaired.contains_key(id)
                || self.active.contains_key(id)
                || self.pending.contains(id)
                || tasks.len() >= 96
            {
                continue;
            }
            let mut addresses = peer.addresses.clone();
            for hint in self.hints.borrow().iter().filter(|h| h.noob_id == *id) {
                for address in &hint.addresses {
                    if let Ok(mut address) = address.parse::<SocketAddr>() {
                        address.set_port(hint.sync_port);
                        if !addresses.contains(&address) {
                            addresses.push(address);
                        }
                    }
                }
            }
            addresses.truncate(16);
            if addresses.is_empty() {
                continue;
            }
            let tls = self.tls.clone();
            let addresses =
                dial::order(self.next_address.entry(id.clone()).or_default(), addresses);
            let id = id.clone();
            self.pending.insert(id.clone());
            let revision = self.revision;
            let stop = self.stop.clone();
            tasks.spawn(async move {
                let result = interruptible(
                    stop,
                    dial::connect(addresses, |address| {
                        let tls = &tls;
                        let id = &id;
                        async move { tls.connect_peer(&address.to_string(), id).await }
                    }),
                )
                .await;
                Completion::Handshake {
                    expected: Some(id),
                    revision,
                    result,
                }
            });
        }
    }
    fn connected(
        &mut self,
        connection: Connection,
        tasks: &mut JoinSet<Completion>,
        session_events: &mpsc::Sender<SessionEvent>,
    ) -> Result<()> {
        let peer = connection.peer_id().to_owned();
        let Some(record) = self.peers.get(&peer) else {
            return Ok(());
        };
        if self.disabled.contains(&peer)
            || self.pairing.contains_key(&peer)
            || self.unpaired.contains_key(&peer)
            || record.identity.fingerprint != connection.peer_fingerprint()
        {
            return Ok(());
        }
        if self.active.contains_key(&peer) {
            return Ok(());
        }
        self.disconnect(&peer);
        self.generation = self.generation.checked_add(1).ok_or(Failure::Internal)?;
        let generation = self.generation;
        let outbox = Outbox::new();
        outbox.control(Message::Paused {
            paused: self.paused,
        })?;
        outbox.control(Message::Device {
            device_name: self.name.clone(),
        })?;
        if self
            .events
            .try_send(Event::Connected {
                peer: peer.clone(),
                generation,
                outbox: outbox.clone(),
            })
            .is_err()
        {
            outbox.close();
            return Ok(());
        }
        self.active.insert(
            peer.clone(),
            Slot {
                generation,
                peer_address: connection.peer_address(),
                paused: false,
                outbox: outbox.clone(),
            },
        );
        let events = session_events.clone();
        tasks.spawn(async move {
            let result = session::run(connection, outbox, peer.clone(), generation, events).await;
            Completion::Session {
                peer,
                generation,
                unpaired: matches!(result, Err(Failure::Unpaired)),
            }
        });
        self.publish();
        Ok(())
    }
    pub async fn run(mut self) -> Result<()> {
        let mut hints = self.hints.clone();
        let mut tasks = JoinSet::new();
        let (session_events, mut incoming) = mpsc::channel(64);
        let mut retry = tokio::time::interval(Duration::from_secs(1));
        retry.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let result = async {
            loop {
                if *self.stop.borrow() {
                    break;
                }
                tokio::select! {
                    _ = self.stop.changed() => break,
                    request = self.requests.recv() => match request {
                        Some(request) => self.request(request),
                        None => break,
                    },
                    next = tasks.join_next(), if !tasks.is_empty() => {
                        match next.ok_or(Failure::Internal)?.map_err(|_| Failure::Internal)? {
                            Completion::Handshake { expected, revision, result } => {
                                if let Some(id) = &expected {
                                    self.pending.remove(id);
                                }
                                if revision == self.revision && matches!(result, Err(Failure::Unpaired))
                                    && let Some(id) = &expected {
                                    self.mark_unpaired(id)?;
                                }
                                if revision == self.revision && let Ok(connection) = result {
                                    let preferred = expected.is_some() == (self.own_id.as_str() < connection.peer_id());
                                    if preferred {
                                        self.connected(connection, &mut tasks, &session_events)?;
                                    } else {
                                        // Give the deterministic direction time to win simultaneous dials.
                                        // Never replace a session already exposed to transfers.
                                        tasks.spawn(async move {
                                            tokio::time::sleep(Duration::from_millis(200)).await;
                                            Completion::Candidate { connection, revision }
                                        });
                                    }
                                }
                            }
                            Completion::Candidate { connection, revision } => {
                                if revision == self.revision {
                                    self.connected(connection, &mut tasks, &session_events)?;
                                }
                            }
                            Completion::Session { peer, generation, unpaired } => {
                                if self.active.get(&peer).is_some_and(|s| s.generation == generation) {
                                    self.disconnect(&peer);
                                    if unpaired {
                                        self.mark_unpaired(&peer)?;
                                    }
                                    self.publish();
                                }
                            }
                        }
                    },
                    Some(event) = incoming.recv() => {
                        let (peer, generation) = match &event {
                            SessionEvent::Message { peer, generation, .. }
                            | SessionEvent::Started { peer, generation, .. }
                            | SessionEvent::Written { peer, generation, .. } => (peer.clone(), *generation),
                        };
                        if self.active.get(&peer).is_none_or(|s| s.generation != generation) {
                            continue;
                        }
                        let event = match event {
                            SessionEvent::Message { peer, generation, message } => {
                                if let Message::Paused { paused } = &message {
                                    if let Some(slot) = self.active.get_mut(&peer) {
                                        slot.paused = *paused;
                                    }
                                    self.publish();
                                }
                                if let Message::Device { device_name } = &message
                                    && let Some(record) = self.peers.get_mut(&peer) {
                                    record.identity.device_name.clone_from(device_name);
                                    self.publish();
                                }
                                Event::Message { peer, generation, message }
                            },
                            SessionEvent::Started { peer, generation, id } => Event::Started { peer, generation, id },
                            SessionEvent::Written { peer, generation, id } => Event::Written { peer, generation, id },
                        };
                        if self.events.try_send(event).is_err() {
                            self.disconnect(&peer);
                            self.publish();
                        }
                    },
                    accepted = self.listener.accept() => {
                        let (stream, _) = accepted?;
                        if tasks.len() < 96 {
                            let tls = self.tls.clone();
                            let revision = self.revision;
                            let stop = self.stop.clone();
                            tasks.spawn(async move {
                                Completion::Handshake {
                                    expected: None,
                                    revision,
                                    result: interruptible(stop, tls.accept(stream)).await,
                                }
                            });
                        }
                    },
                    _ = hints.changed() => {
                        self.refresh_discovery(&hints.borrow_and_update());
                        self.dial(&mut tasks);
                    },
                    _ = retry.tick() => self.dial(&mut tasks),
                }
            }
            Ok(())
        }.await;
        self.requests.close();
        for slot in self.active.values() {
            slot.outbox.close();
        }
        // Sessions close and await their framing readers. Handshakes observe the stop signal.
        while tasks.join_next().await.is_some() {}
        self.active.clear();
        self.publish();
        result
    }
}
fn make_tls(identity: &Identity, peers: &BTreeMap<String, TrustedPeer>) -> Result<TlsConfig> {
    TlsConfig::with_peers(
        identity,
        &peers
            .values()
            .map(|p| p.identity.certificate.clone())
            .collect::<Vec<_>>(),
    )
}

async fn interruptible<T>(
    mut stop: watch::Receiver<bool>,
    work: impl std::future::Future<Output = Result<T>>,
) -> Result<T> {
    if *stop.borrow() {
        return Err(Failure::Cancelled);
    }
    tokio::select! {
        _ = stop.changed() => Err(Failure::Cancelled),
        result = work => result,
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
