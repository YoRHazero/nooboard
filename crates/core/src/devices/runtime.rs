use super::{model::DeviceState, pairing};
use crate::{
    Error, PairingSession, PairingStage, PeerSettings, Result,
    configuration::runtime::{Change, Handle as Configuration},
    runtime::message::Reply,
};
use nooboard_network::{Network, NetworkEvent, PairingId};
use tokio::sync::{mpsc, watch};
pub(crate) enum Request {
    Refresh {
        reply: Reply<()>,
    },
    Begin {
        address: String,
        expected: Option<String>,
        reply: Reply<()>,
    },
    Accept {
        id: String,
        reply: Reply<()>,
    },
    Code {
        id: String,
        code: String,
        reply: Reply<()>,
    },
    Dismiss {
        id: String,
        reply: Reply<()>,
    },
    Unpair {
        id: String,
        reply: Reply<()>,
    },
    Configure {
        id: String,
        settings: PeerSettings,
        reply: Reply<()>,
    },
    #[cfg(any(test, feature = "diagnostics"))]
    Trust {
        fixture: crate::PeerFixture,
        reply: Reply<String>,
    },
}
pub(crate) struct Runtime {
    requests: mpsc::Receiver<Request>,
    events: mpsc::Receiver<NetworkEvent>,
    state: watch::Sender<DeviceState>,
    network: Network,
    config: Configuration,
    selected: Option<PairingId>,
    selected_terminal: bool,
    expected: Option<String>,
    failure: Option<crate::PairingFailure>,
    expires_at_ms: i64,
    applied_revision: Option<u64>,
    applied: std::collections::BTreeMap<String, nooboard_network::TrustedPeer>,
}
pub(crate) struct Channels {
    pub requests: mpsc::Sender<Request>,
    pub events: mpsc::Sender<NetworkEvent>,
    pub state: watch::Receiver<DeviceState>,
}
pub(crate) fn create(network: Network, config: Configuration) -> (Channels, Runtime) {
    let applied = config
        .current()
        .peers
        .into_iter()
        .map(|(id, peer)| (id, peer.trusted))
        .collect();
    let (requests, inbox) = mpsc::channel(32);
    let (events, receiver) = mpsc::channel(32);
    let (state, snapshot) = watch::channel(DeviceState::default());
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
            network,
            config,
            selected: None,
            selected_terminal: false,
            expected: None,
            failure: None,
            expires_at_ms: 0,
            applied,
            applied_revision: None,
        },
    )
}
impl Runtime {
    pub async fn run(mut self, mut stop: watch::Receiver<bool>) -> Result<()> {
        let mut status = self.network.subscribe();
        let mut config = self.config.state.clone();
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(5));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        if let Err(error) = self.reconcile().await {
            self.fault(&error);
        }
        self.refresh_addresses().await;
        self.publish();
        loop {
            if *stop.borrow() {
                break;
            }
            tokio::select! {
                _ = stop.changed() => break,
                Some(event) = self.events.recv() => {
                    if let Err(error) = self.event(event).await { self.fault(&error); }
                    self.publish();
                },
                request = self.requests.recv() => match request {
                    Some(request) => { self.command(request).await; self.publish(); },
                    None => break,
                },
                changed = config.changed() => {
                    if changed.is_err() { return Err(Error::Stopped); }
                    if let Err(error) = self.refresh_peer_names().await { self.fault(&error); }
                    if let Err(error) = self.reconcile().await { self.fault(&error); }
                    self.publish();
                },
                changed = status.changed() => {
                    if changed.is_err() { return Err(Error::Stopped); }
                    if let Err(error) = self.refresh_peer_names().await { self.fault(&error); }
                    self.publish();
                },
                _ = tick.tick() => {
                    self.refresh_addresses().await;
                    if let Err(error) = self.refresh_peer_names().await { self.fault(&error); }
                    if let Err(error) = self.reconcile().await { self.fault(&error); }
                    self.publish();
                },
            }
        }
        // Pairing is still alive here; cancel unfinished user interactions before network shutdown.
        for p in self.network.status().pairings {
            if !matches!(p.stage, PairingStage::Completed | PairingStage::Failed) {
                let _ = self.network.cancel_pairing(p.id).await;
            }
        }
        Ok(())
    }
    fn fault(&self, e: &Error) {
        self.state.send_modify(|s| s.fault = Some(e.to_string()));
    }
    fn selected(&self, id: &str) -> Result<PairingId> {
        self.network
            .status()
            .pairings
            .into_iter()
            .find(|p| p.id.as_str() == id)
            .map(|p| p.id)
            .ok_or(Error::NotFound)
    }
    fn active(&self) -> bool {
        self.selected.as_ref().is_some_and(|id| {
            self.network
                .status()
                .pairings
                .iter()
                .find(|p| &p.id == id)
                .map_or(!self.selected_terminal, |p| {
                    !matches!(p.stage, PairingStage::Completed | PairingStage::Failed)
                })
        })
    }
    async fn reconcile(&mut self) -> Result<()> {
        let config = self.config.current();
        if self.applied_revision == Some(config.revision) {
            return Ok(());
        }
        for id in self
            .applied
            .keys()
            .filter(|id| !config.peers.contains_key(*id))
            .cloned()
            .collect::<Vec<_>>()
        {
            self.network.revoke_peer(id.clone()).await?;
            self.applied.remove(&id);
        }
        for (id, peer) in &config.peers {
            let trusted = pairing::trusted(peer).await?;
            if self.applied.get(id) != Some(&trusted) {
                self.network.trust_peer(trusted.clone()).await?;
                self.applied.insert(id.clone(), trusted);
            }
        }
        self.network
            .enable_discovery(config.settings.discoverable)
            .await?;
        self.applied_revision = Some(config.revision);
        self.state.send_modify(|s| {
            s.effective_revision = config.revision;
            s.fault = None;
        });
        Ok(())
    }
    async fn refresh_peer_names(&mut self) -> Result<()> {
        let config = self.config.current();
        // Connection names come from the authenticated protocol. Discovery names
        // are only hints and must never replace a saved device's name.
        let names =
            self.network
                .status()
                .connections
                .into_iter()
                .filter(|connection| connection.connected)
                .filter(|connection| {
                    config.peers.get(&connection.peer).is_some_and(|peer| {
                        peer.trusted.identity.device_name != connection.device_name
                    })
                })
                .map(|connection| (connection.peer, connection.device_name))
                .collect::<Vec<_>>();
        if names.is_empty() {
            return Ok(());
        }
        self.config.change(Change::PeerNames(names.clone())).await?;
        // Metadata already came from network; do not reapply trust just to echo
        // the name back into the connection runtime.
        for (id, name) in names {
            if let Some(peer) = self.applied.get_mut(&id) {
                peer.identity.device_name = name;
            }
        }
        Ok(())
    }
    async fn refresh_addresses(&self) {
        let status = self.network.status();
        let result = self.network.local_addresses().await;
        self.state.send_modify(|s| {
            s.local.sync_port = status.listen_address.port();
            s.local.pairing_port = status.pairing_address.port();
            match result {
                Ok(addresses) => {
                    s.local.addresses = addresses;
                    s.local.error = None;
                }
                Err(e) => s.local.error = Some(e.to_string()),
            }
        });
    }
    fn publish(&mut self) {
        let net = self.network.status();
        if let Some(pairing) = self
            .selected
            .as_ref()
            .and_then(|id| net.pairings.iter().find(|p| &p.id == id))
        {
            self.selected_terminal = matches!(
                pairing.stage,
                PairingStage::Completed | PairingStage::Failed
            );
        }
        let session =
            self.selected
                .as_ref()
                .and_then(|id| net.pairings.iter().find(|p| &p.id == id))
                .map(|p| PairingSession {
                    id: p.id.as_str().into(),
                    incoming: p.incoming,
                    device_name: p
                        .peer
                        .as_ref()
                        .map_or_else(String::new, |p| p.device_name.clone()),
                    noob_id: p.peer.as_ref().map(|p| p.id.clone()),
                    stage: p.stage,
                    code: p.code.clone(),
                    expires_at_ms: self.expires_at_ms,
                    attempts_left: p.attempts_left,
                    error: self
                        .failure
                        .clone()
                        .or_else(|| p.error.map(pairing::failure))
                        .or_else(|| {
                            (p.stage == PairingStage::EnteringCode && p.attempts_left < 3)
                                .then_some(crate::PairingFailure::IncorrectCode {
                                    remaining: p.attempts_left,
                                })
                        }),
                });
        self.state.send_modify(|s| {
            s.onboarding.nearby = net.nearby;
            s.onboarding.pairing_address = net.pairing_address.to_string();
            s.onboarding.session = session;
        });
    }
    async fn event(&mut self, event: NetworkEvent) -> Result<()> {
        match event {
            NetworkEvent::PairingOffered { id, .. } => {
                if self.active() {
                    self.network.cancel_pairing(id).await?;
                } else {
                    self.selected = Some(id);
                    self.selected_terminal = false;
                    self.expected = None;
                    self.failure = None;
                    self.expires_at_ms = crate::history::now_ms() + 120_000;
                }
            }
            NetworkEvent::PairingVerified { id, peer } => {
                let result = pairing::verified(
                    &self.network,
                    &self.config,
                    id,
                    peer,
                    self.expected.as_deref(),
                )
                .await;
                if let Err(e) = &result {
                    self.failure = Some(match e {
                        Error::Storage(_) => crate::PairingFailure::Storage,
                        Error::AlreadyPaired => crate::PairingFailure::IdentityChanged,
                        _ => crate::PairingFailure::Network(crate::PairingError::Disconnected),
                    });
                }
                result?;
            }
            _ => return Err(Error::Internal),
        }
        Ok(())
    }
    async fn command(&mut self, request: Request) {
        match request {
            Request::Refresh { reply } => {
                let result = self.network.refresh_discovery().await.map_err(Into::into);
                self.refresh_addresses().await;
                let _ = reply.send(result);
            }
            Request::Begin {
                address,
                expected,
                reply,
            } => {
                let result = async {
                    if self.active() {
                        return Err(Error::Busy);
                    }
                    if expected.as_ref().is_some_and(|id| {
                        id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit())
                    }) {
                        return Err(Error::Configuration);
                    }
                    let id = self
                        .network
                        .start_pairing(pairing::addresses(&address).await?)
                        .await?;
                    self.selected = Some(id);
                    self.selected_terminal = false;
                    self.expected = expected;
                    self.failure = None;
                    self.expires_at_ms = crate::history::now_ms() + 120_000;
                    Ok(())
                }
                .await;
                self.publish();
                let _ = reply.send(result);
            }
            Request::Accept { id, reply } => {
                let result = async {
                    self.network.accept_pairing(self.selected(&id)?).await?;
                    Ok(())
                }
                .await;
                let _ = reply.send(result);
            }
            Request::Code { id, code, reply } => {
                let result = async {
                    self.network
                        .submit_pairing_code(self.selected(&id)?, code)
                        .await?;
                    Ok(())
                }
                .await;
                let _ = reply.send(result);
            }
            Request::Dismiss { id, reply } => {
                let result = async {
                    let id = self.selected(&id)?;
                    if self.network.status().pairings.iter().any(|p| {
                        p.id == id
                            && !matches!(p.stage, PairingStage::Completed | PairingStage::Failed)
                    }) {
                        self.network.cancel_pairing(id.clone()).await?;
                    }
                    if self.selected.as_ref() == Some(&id) {
                        self.selected = None;
                        self.failure = None;
                    }
                    Ok(())
                }
                .await;
                self.publish();
                let _ = reply.send(result);
            }
            Request::Unpair { id, reply } => {
                let result = async {
                    self.config.change(Change::RemovePeer(id.clone())).await?;
                    self.network.revoke_peer(id.clone()).await?;
                    self.applied.remove(&id);
                    Ok(())
                }
                .await;
                let _ = reply.send(result);
            }
            Request::Configure {
                id,
                settings,
                reply,
            } => {
                let result = async {
                    settings.validate()?;
                    if let Some(address) = &settings.address {
                        pairing::addresses(address).await?;
                    }
                    self.config
                        .change(Change::ConfigurePeer(id, settings))
                        .await?;
                    self.reconcile().await
                }
                .await;
                let _ = reply.send(result);
            }
            #[cfg(any(test, feature = "diagnostics"))]
            Request::Trust { fixture, reply } => {
                let result = async {
                    let addresses = match &fixture.address {
                        Some(v) => pairing::addresses(v).await?,
                        None => vec![],
                    };
                    let peer = nooboard_network::TrustedPeer {
                        identity: nooboard_network::PublicIdentity {
                            id: fixture.noob_id.clone(),
                            fingerprint: fixture.confirmed_fingerprint,
                            certificate: fixture.certificate,
                            device_name: fixture.device_name,
                        },
                        addresses,
                    };
                    self.config.change(Change::SavePeer(peer.clone())).await?;
                    self.network.trust_peer(peer.clone()).await?;
                    self.applied.insert(fixture.noob_id.clone(), peer);
                    Ok(fixture.noob_id)
                }
                .await;
                let _ = reply.send(result);
            }
        }
    }
}
