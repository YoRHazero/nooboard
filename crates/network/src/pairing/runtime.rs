use super::{
    Contact, Control, Endpoint, Error as PairError, Event, PairingId, PairingStatus, Stage,
};
use crate::{
    connections,
    error::{ErrorKind, Failure, InternalResult as Result},
    event::NetworkEvent,
    identity::{
        PublicIdentity, TrustedPeer,
        material::{fingerprint, new_session_id, noob_id},
    },
};
use std::{collections::BTreeMap, net::SocketAddr};
use tokio::sync::{mpsc, oneshot, watch};

pub(crate) enum Request {
    Start {
        addresses: Vec<SocketAddr>,
        reply: oneshot::Sender<Result<PairingId>>,
    },
    Accept {
        id: PairingId,
        reply: oneshot::Sender<Result<()>>,
    },
    Code {
        id: PairingId,
        code: zeroize::Zeroizing<String>,
        reply: oneshot::Sender<Result<()>>,
    },
    Cancel {
        id: PairingId,
        reply: oneshot::Sender<Result<()>>,
    },
    Saved {
        id: PairingId,
        saved: bool,
        reply: oneshot::Sender<Result<()>>,
    },
}
pub(crate) struct Handle {
    pub requests: mpsc::Sender<Request>,
    pub status: watch::Receiver<Vec<PairingStatus>>,
    pub address: SocketAddr,
}
struct Pending {
    peer: TrustedPeer,
    saved: oneshot::Sender<bool>,
}
pub(crate) struct Runtime {
    endpoint: Endpoint,
    events: mpsc::Receiver<Event>,
    requests: mpsc::Receiver<Request>,
    status: watch::Sender<Vec<PairingStatus>>,
    inbox: mpsc::Sender<NetworkEvent>,
    stop: watch::Receiver<bool>,
    connections: mpsc::Sender<connections::runtime::Request>,
    sessions: BTreeMap<PairingId, PairingStatus>,
    controls: BTreeMap<PairingId, Control>,
    pending: BTreeMap<PairingId, Pending>,
}
impl Runtime {
    pub async fn bind(
        address: SocketAddr,
        local: Contact,
        capacity: usize,
        inbox: mpsc::Sender<NetworkEvent>,
        stop: watch::Receiver<bool>,
        connections: mpsc::Sender<connections::runtime::Request>,
    ) -> Result<(Self, Handle)> {
        let (tx, events) = mpsc::channel(64);
        let endpoint = Endpoint::bind(&address.to_string(), local, tx).await?;
        let address = endpoint.address.parse().map_err(|_| Failure::Internal)?;
        let (sender, requests) = mpsc::channel(capacity);
        let (status, rx) = watch::channel(vec![]);
        Ok((
            Self {
                endpoint,
                events,
                requests,
                status,
                inbox,
                stop,
                connections,
                sessions: BTreeMap::new(),
                controls: BTreeMap::new(),
                pending: BTreeMap::new(),
            },
            Handle {
                requests: sender,
                status: rx,
                address,
            },
        ))
    }
    fn publish(&self) {
        self.status
            .send_replace(self.sessions.values().cloned().collect());
    }
    fn retire(&mut self, id: &PairingId, error: Option<ErrorKind>) -> bool {
        let Some(control) = self.controls.remove(id) else {
            return false;
        };
        control.cancel();
        self.pending.remove(id);
        if let Some(status) = self.sessions.get_mut(id) {
            status.code = None;
            status.stage = if error.is_some() {
                Stage::Failed
            } else {
                Stage::Completed
            };
            status.error = error;
        }
        while self.sessions.len() > 32 {
            let old = self
                .sessions
                .iter()
                .find(|(_, s)| matches!(s.stage, Stage::Completed | Stage::Failed))
                .map(|(id, _)| id.clone());
            if let Some(old) = old {
                self.sessions.remove(&old);
            } else {
                break;
            }
        }
        true
    }
    async fn request(&mut self, request: Request) {
        match request {
            Request::Start { addresses, reply } => {
                if reply.is_closed() {
                    return;
                }
                let result = (|| {
                    if self.controls.len() >= 8 {
                        return Err(Failure::Busy);
                    }
                    if addresses.is_empty()
                        || addresses.len() > 16
                        || addresses.iter().any(|a| {
                            a.port() == 0 || a.ip().is_unspecified() || a.ip().is_multicast()
                        })
                    {
                        return Err(Failure::InvalidArgument("pairing addresses"));
                    }
                    let id = PairingId(new_session_id()?);
                    let control = self
                        .endpoint
                        .connect(
                            id.0.clone(),
                            addresses.iter().map(ToString::to_string).collect(),
                        )
                        .map_err(convert)?;
                    self.controls.insert(id.clone(), control);
                    self.sessions.insert(
                        id.clone(),
                        PairingStatus {
                            id: id.clone(),
                            peer: None,
                            incoming: false,
                            stage: Stage::Requesting,
                            code: None,
                            attempts_left: 3,
                            error: None,
                        },
                    );
                    Ok(id)
                })();
                // A cancelled start must not leave an unobservable pairing running.
                if let Err(Ok(id)) = reply.send(result) {
                    self.retire(&id, Some(ErrorKind::Cancelled));
                }
            }
            Request::Accept { id, reply } => {
                if !reply.is_closed() {
                    let result = if self
                        .sessions
                        .get(&id)
                        .is_some_and(|s| s.incoming && s.stage == Stage::AwaitingApproval)
                    {
                        self.controls
                            .get(&id)
                            .ok_or(Failure::NotFound)
                            .and_then(|c| c.accept().map_err(convert))
                    } else {
                        Err(Failure::InvalidArgument("pairing stage"))
                    };
                    let _ = reply.send(result);
                }
            }
            Request::Code { id, code, reply } => {
                if !reply.is_closed() {
                    let result = if self
                        .sessions
                        .get(&id)
                        .is_some_and(|s| s.stage == Stage::EnteringCode)
                    {
                        self.controls
                            .get(&id)
                            .ok_or(Failure::NotFound)
                            .and_then(|c| c.code(code.to_string()).map_err(convert))
                    } else {
                        Err(Failure::InvalidArgument("pairing stage"))
                    };
                    let _ = reply.send(result);
                }
            }
            Request::Cancel { id, reply } => {
                if !reply.is_closed() {
                    let retired = self.retire(&id, Some(ErrorKind::Cancelled));
                    let _ = reply.send(if retired {
                        Ok(())
                    } else {
                        Err(Failure::NotFound)
                    });
                }
            }
            Request::Saved { id, saved, reply } => {
                if reply.is_closed() {
                    return;
                }
                let result = if let Some(pending) = self.pending.remove(&id) {
                    if saved {
                        let (tx, rx) = oneshot::channel();
                        let result = tokio::select! {
                            _ = self.stop.changed() => Err(Failure::Closed),
                            result = async {
                                self.connections.send(connections::runtime::Request::Trust {
                                    peer: pending.peer,
                                    reply: tx,
                                }).await.map_err(|_| Failure::Closed)?;
                                rx.await.map_err(|_| Failure::Closed)?
                            } => result,
                        };
                        let _ = pending.saved.send(result.is_ok());
                        result
                    } else {
                        let _ = pending.saved.send(false);
                        Ok(())
                    }
                } else {
                    Err(Failure::NotFound)
                };
                let _ = reply.send(result);
            }
        }
        self.publish();
    }
    fn event(&mut self, event: Event) {
        match event {
            Event::Offered {
                id,
                peer,
                control,
                incoming,
            } => {
                let id = PairingId(id);
                // Ignore stale events from already-cancelled outgoing sessions.
                if self
                    .sessions
                    .get(&id)
                    .is_some_and(|s| matches!(s.stage, Stage::Failed | Stage::Completed))
                {
                    control.cancel();
                    return;
                }
                if self.controls.len() >= 8 && !self.controls.contains_key(&id) {
                    control.cancel();
                    return;
                }
                let Ok(public) = public(peer) else {
                    control.cancel();
                    return;
                };
                self.controls.insert(id.clone(), control);
                self.sessions
                    .entry(id.clone())
                    .or_insert(PairingStatus {
                        id: id.clone(),
                        peer: None,
                        incoming,
                        stage: Stage::AwaitingApproval,
                        code: None,
                        attempts_left: 3,
                        error: None,
                    })
                    .peer = Some(public.clone());
                if incoming
                    && self
                        .inbox
                        .try_send(NetworkEvent::PairingOffered {
                            id: id.clone(),
                            peer: public,
                        })
                        .is_err()
                {
                    self.retire(&id, Some(ErrorKind::Busy));
                }
            }
            Event::Progress {
                id,
                stage,
                code,
                attempts_left,
            } => {
                let id = PairingId(id);
                if self.controls.contains_key(&id)
                    && let Some(status) = self.sessions.get_mut(&id)
                {
                    status.stage = stage;
                    status.code = code;
                    status.attempts_left = attempts_left;
                }
            }
            Event::Verified {
                id,
                peer,
                address,
                saved,
            } => {
                let id = PairingId(id);
                let record = public(peer).and_then(|identity| {
                    Ok(TrustedPeer {
                        identity,
                        addresses: vec![address.parse().map_err(|_| Failure::Protocol)?],
                    })
                });
                if !self.controls.contains_key(&id) {
                    let _ = saved.send(false);
                    return;
                }
                if let Ok(peer) = record {
                    if self
                        .inbox
                        .try_send(NetworkEvent::PairingVerified {
                            id: id.clone(),
                            peer: peer.clone(),
                        })
                        .is_ok()
                    {
                        self.pending.insert(id, Pending { peer, saved });
                    } else {
                        let _ = saved.send(false);
                        self.retire(&id, Some(ErrorKind::Busy));
                    }
                } else {
                    let _ = saved.send(false);
                    self.retire(&id, Some(ErrorKind::Protocol));
                }
            }
            Event::Completed { id } => {
                let id = PairingId(id);
                if self.controls.contains_key(&id) {
                    self.retire(&id, None);
                }
            }
            Event::Failed { id, error } => {
                let id = PairingId(id);
                if self.controls.contains_key(&id) {
                    self.retire(&id, Some(convert(error).kind()));
                }
            }
        }
        self.publish();
    }
    pub async fn run(mut self) -> Result<()> {
        loop {
            if *self.stop.borrow() {
                break;
            }
            tokio::select! {
                _ = self.stop.changed() => break,
                event = self.events.recv() => match event { Some(event) => self.event(event), None => break },
                request = self.requests.recv() => match request { Some(request) => self.request(request).await, None => break },
            }
        }
        self.requests.close();
        for id in self.controls.keys().cloned().collect::<Vec<_>>() {
            self.retire(&id, Some(ErrorKind::Stopped));
        }
        self.publish();
        self.endpoint.shutdown().await
    }
}
fn public(peer: Contact) -> Result<PublicIdentity> {
    let identity = PublicIdentity {
        id: noob_id(&peer.certificate)?,
        fingerprint: fingerprint(&peer.certificate),
        certificate: peer.certificate,
        device_name: peer.device_name,
    };
    identity.validate()?;
    Ok(identity)
}
fn convert(error: PairError) -> Failure {
    match error {
        PairError::Timeout => Failure::Timeout,
        PairError::Cancelled => Failure::Cancelled,
        PairError::Busy => Failure::Busy,
        PairError::Code => Failure::InvalidArgument("pairing code"),
        PairError::Connect | PairError::Disconnected => Failure::Disconnected,
        PairError::Storage => Failure::Credentials,
        _ => Failure::Protocol,
    }
}
