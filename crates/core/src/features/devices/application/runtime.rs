use super::{pairing, state::DeviceState};
use crate::contracts::network::{NetworkEvent, PairingId};
use crate::{
    Error, LocalNetwork, OnboardingSnapshot, PairingSession, PairingStage, PeerSettings, Result,
    configuration::{domain::change::Change, ports::settings::ConfigurationPort},
    message::Reply,
    publish::Publish,
};
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
    network: std::sync::Arc<dyn crate::features::devices::ports::network::NetworkPort>,
    config: std::sync::Arc<dyn ConfigurationPort>,
    selected: Option<PairingId>,
    /// Whether the selected pairing had ended when the network last reported it; the
    /// network may later drop the record, and this keeps the outcome.
    selected_terminal: bool,
    expected: Option<String>,
    failure: Option<crate::PairingFailure>,
    expires_at_ms: i64,
    applied_revision: Option<u64>,
    applied: std::collections::BTreeMap<String, crate::contracts::network::TrustedPeer>,
    local: LocalNetwork,
    /// Set while discovery is wanted but unavailable; see `discover`.
    discovery_error: Option<String>,
    fault: Option<String>,
    effective_revision: u64,
}
pub(crate) struct Channels {
    pub requests: mpsc::Sender<Request>,
    pub events: mpsc::Sender<NetworkEvent>,
    pub state: watch::Receiver<DeviceState>,
}
pub(crate) fn create(
    network: std::sync::Arc<dyn crate::features::devices::ports::network::NetworkPort>,
    config: std::sync::Arc<dyn ConfigurationPort>,
) -> (Channels, Runtime) {
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
            local: LocalNetwork::default(),
            discovery_error: None,
            fault: None,
            effective_revision: 0,
        },
    )
}
impl Runtime {
    pub async fn run(mut self, mut stop: watch::Receiver<bool>) -> Result<()> {
        let mut status = self.network.subscribe();
        let mut config = self.config.subscribe();
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
                    // `command` publishes its effects before each reply.
                    Some(request) => self.command(request).await,
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
                    self.observe_selected();
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
    /// Published at once: a later successful reconcile in the same step clears the
    /// fault, and that must not hide a failure that already happened.
    fn fault(&mut self, e: &Error) {
        self.fault = Some(e.to_string());
        self.publish();
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
    /// Records how the selected pairing ended, as the network reports it.
    fn observe_selected(&mut self) {
        if let Some(pairing) = self.selected.as_ref().and_then(|id| {
            self.network
                .status()
                .pairings
                .into_iter()
                .find(|p| &p.id == id)
        }) {
            self.selected_terminal = matches!(
                pairing.stage,
                PairingStage::Completed | PairingStage::Failed
            );
        }
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
            let trusted = pairing::trusted(self.network.as_ref(), peer).await?;
            if self.applied.get(id) != Some(&trusted) {
                self.network.trust_peer(trusted.clone()).await?;
                self.applied.insert(id.clone(), trusted);
            }
        }
        self.discover(config.settings.discoverable, false).await?;
        self.applied_revision = Some(config.revision);
        self.effective_revision = config.revision;
        self.fault = None;
        Ok(())
    }
    /// Applies the discoverable setting and, for a search, looks for nearby devices again.
    /// Discovery is optional: devices can still be added by address. While it is wanted
    /// but unavailable, that is onboarding state rather than an error, and a later search
    /// retries starting it. Turning it off must still succeed.
    async fn discover(&mut self, discoverable: bool, search: bool) -> Result<()> {
        let result = async {
            self.network.enable_discovery(discoverable).await?;
            if search {
                self.network.refresh_discovery().await?;
            }
            Ok(())
        }
        .await;
        if !discoverable {
            self.discovery_error = None;
            return result;
        }
        self.discovery_error = result.err().map(|error| error.to_string());
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
    async fn refresh_addresses(&mut self) {
        let status = self.network.status();
        let result = self.network.local_addresses().await;
        self.local.sync_port = status.listen_address.port();
        self.local.pairing_port = status.pairing_address.port();
        match result {
            Ok(addresses) => {
                self.local.addresses = addresses;
                self.local.error = None;
            }
            Err(e) => self.local.error = Some(e.to_string()),
        }
    }
    /// The only projection of this runtime's state to observers.
    fn publish(&self) {
        let net = self.network.status();
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
        self.state.publish(DeviceState {
            onboarding: OnboardingSnapshot {
                nearby: net.nearby,
                pairing_address: net.pairing_address.to_string(),
                discovery_error: self.discovery_error.clone(),
                session,
            },
            local: self.local.clone(),
            fault: self.fault.clone(),
            effective_revision: self.effective_revision,
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
                    self.expires_at_ms = crate::clock::now_ms() + 120_000;
                }
            }
            NetworkEvent::PairingVerified { id, peer } => {
                let result = pairing::verified(
                    self.network.as_ref(),
                    self.config.as_ref(),
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
    /// A command's effects are published before its reply, so callers observe them on return.
    fn reply<T>(&mut self, reply: Reply<T>, result: Result<T>) {
        self.publish();
        let _ = reply.send(result);
    }
    async fn command(&mut self, request: Request) {
        match request {
            Request::Refresh { reply } => {
                let discoverable = self.config.current().settings.discoverable;
                let result = self.discover(discoverable, true).await;
                self.refresh_addresses().await;
                self.reply(reply, result);
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
                        .start_pairing(self.network.resolve(&address).await?)
                        .await?;
                    self.selected = Some(id);
                    self.selected_terminal = false;
                    self.expected = expected;
                    self.failure = None;
                    self.expires_at_ms = crate::clock::now_ms() + 120_000;
                    Ok(())
                }
                .await;
                self.reply(reply, result);
            }
            Request::Accept { id, reply } => {
                let result = async {
                    self.network.accept_pairing(self.selected(&id)?).await?;
                    Ok(())
                }
                .await;
                self.reply(reply, result);
            }
            Request::Code { id, code, reply } => {
                let result = async {
                    self.network
                        .submit_pairing_code(self.selected(&id)?, code)
                        .await?;
                    Ok(())
                }
                .await;
                self.reply(reply, result);
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
                self.reply(reply, result);
            }
            Request::Unpair { id, reply } => {
                let result = async {
                    self.config.change(Change::RemovePeer(id.clone())).await?;
                    self.network.revoke_peer(id.clone()).await?;
                    self.applied.remove(&id);
                    Ok(())
                }
                .await;
                self.reply(reply, result);
            }
            Request::Configure {
                id,
                settings,
                reply,
            } => {
                let result = async {
                    settings.validate()?;
                    if let Some(address) = &settings.address {
                        self.network.resolve(address).await?;
                    }
                    self.config
                        .change(Change::ConfigurePeer(id, settings))
                        .await?;
                    self.reconcile().await
                }
                .await;
                self.reply(reply, result);
            }
            #[cfg(any(test, feature = "diagnostics"))]
            Request::Trust { fixture, reply } => {
                let result = async {
                    let addresses = match &fixture.address {
                        Some(v) => self.network.resolve(v).await?,
                        None => vec![],
                    };
                    let peer = crate::contracts::network::TrustedPeer {
                        identity: crate::contracts::network::PublicIdentity {
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
                self.reply(reply, result);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Settings;
    use crate::configuration::{
        application::runtime as configuration, domain::model::Configuration,
    };
    use crate::contracts::{
        PortFuture,
        network::{LocalAddress, NetworkStatus, TrustedPeer},
    };
    use crate::features::devices::ports::network::NetworkPort;
    use nooboard_network::Options;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use tokio::sync::oneshot;

    /// A real network whose reported status and discovery outcome the test controls.
    struct ScriptedNetwork {
        inner: Arc<dyn NetworkPort>,
        status: watch::Sender<NetworkStatus>,
        discovery_fails: AtomicBool,
    }
    impl ScriptedNetwork {
        fn discovery(&self) -> PortFuture<'_, ()> {
            let fails = self.discovery_fails.load(Ordering::SeqCst);
            Box::pin(async move { if fails { Err(Error::Internal) } else { Ok(()) } })
        }
    }
    impl NetworkPort for ScriptedNetwork {
        fn status(&self) -> NetworkStatus {
            self.status.borrow().clone()
        }
        fn subscribe(&self) -> watch::Receiver<NetworkStatus> {
            self.status.subscribe()
        }
        fn refresh_discovery(&self) -> PortFuture<'_, ()> {
            self.discovery()
        }
        fn enable_discovery(&self, _enabled: bool) -> PortFuture<'_, ()> {
            self.discovery()
        }
        fn local_addresses(&self) -> PortFuture<'_, Vec<LocalAddress>> {
            self.inner.local_addresses()
        }
        fn trust_peer(&self, peer: TrustedPeer) -> PortFuture<'_, ()> {
            self.inner.trust_peer(peer)
        }
        fn revoke_peer(&self, peer: String) -> PortFuture<'_, ()> {
            self.inner.revoke_peer(peer)
        }
        fn start_pairing(&self, addresses: Vec<std::net::SocketAddr>) -> PortFuture<'_, PairingId> {
            self.inner.start_pairing(addresses)
        }
        fn accept_pairing(&self, id: PairingId) -> PortFuture<'_, ()> {
            self.inner.accept_pairing(id)
        }
        fn cancel_pairing(&self, id: PairingId) -> PortFuture<'_, ()> {
            self.inner.cancel_pairing(id)
        }
        fn submit_pairing_code(&self, id: PairingId, code: String) -> PortFuture<'_, ()> {
            self.inner.submit_pairing_code(id, code)
        }
        fn complete_pairing(&self, id: PairingId, saved: bool) -> PortFuture<'_, ()> {
            self.inner.complete_pairing(id, saved)
        }
        fn resolve<'a>(&'a self, address: &'a str) -> PortFuture<'a, Vec<std::net::SocketAddr>> {
            self.inner.resolve(address)
        }
    }
    fn options() -> Options {
        Options {
            listen: "127.0.0.1:0".parse().unwrap(),
            pairing_listen: "127.0.0.1:0".parse().unwrap(),
            discovery: false,
            ..Default::default()
        }
    }
    async fn scripted(
        settings: Settings,
    ) -> (
        crate::tests::TestNetworkService,
        nooboard_network::Network,
        Arc<ScriptedNetwork>,
        Channels,
        Runtime,
    ) {
        let (owner, network, _events) = crate::tests::start_network(options()).await;
        let scripted = Arc::new(ScriptedNetwork {
            inner: crate::features::devices::adapters::network::port(network.clone()),
            status: watch::channel(network.status()).0,
            discovery_fails: AtomicBool::new(false),
        });
        let (_stop, stopped) = watch::channel(false);
        let (config, _config_runtime) =
            configuration::channel(Configuration::new(settings), stopped);
        let (channels, runtime) = create(scripted.clone(), Arc::new(config));
        (owner, network, scripted, channels, runtime)
    }
    async fn refresh(runtime: &mut Runtime) -> Result<()> {
        let (reply, result) = oneshot::channel();
        runtime.command(Request::Refresh { reply }).await;
        result.await.unwrap()
    }

    #[tokio::test]
    async fn unavailable_discovery_is_onboarding_state_that_a_search_retries() {
        let (owner, _network, scripted, channels, mut runtime) = scripted(Settings {
            discoverable: true,
            ..Settings::default()
        })
        .await;
        let mut state = channels.state;
        scripted.discovery_fails.store(true, Ordering::SeqCst);
        // Pairing by address still works, so applying the settings is not blocked.
        runtime.reconcile().await.unwrap();
        assert_eq!(
            runtime.applied_revision,
            Some(runtime.config.current().revision)
        );
        runtime.publish();
        {
            let current = state.borrow_and_update();
            assert!(current.onboarding.discovery_error.is_some());
            assert!(current.fault.is_none(), "discovery failure became a fault");
        }
        // A search answers normally and keeps showing why nothing is found.
        refresh(&mut runtime).await.unwrap();
        assert!(
            state
                .borrow_and_update()
                .onboarding
                .discovery_error
                .is_some()
        );
        // Once discovery can start, the next search clears the state.
        scripted.discovery_fails.store(false, Ordering::SeqCst);
        refresh(&mut runtime).await.unwrap();
        assert!(
            state
                .borrow_and_update()
                .onboarding
                .discovery_error
                .is_none()
        );
        owner.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn turning_discovery_off_clears_its_state_and_reports_failures() {
        let (owner, _network, scripted, channels, mut runtime) = scripted(Settings {
            discoverable: true,
            ..Settings::default()
        })
        .await;
        scripted.discovery_fails.store(true, Ordering::SeqCst);
        runtime.discover(true, false).await.unwrap();
        assert!(runtime.discovery_error.is_some());
        assert!(runtime.discover(false, false).await.is_err());
        assert!(runtime.discovery_error.is_none());
        runtime.publish();
        assert!(channels.state.borrow().onboarding.discovery_error.is_none());
        owner.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn an_ended_pairing_stays_ended_after_the_network_drops_its_record() {
        let (owner, network, scripted, _channels, mut runtime) = scripted(Settings {
            discoverable: false,
            ..Settings::default()
        })
        .await;
        let (reply, result) = oneshot::channel();
        runtime
            .command(Request::Begin {
                address: "127.0.0.1:1".into(),
                expected: None,
                reply,
            })
            .await;
        result.await.unwrap().unwrap();
        // Selected but not yet reported by the network: the pairing is in progress.
        assert!(runtime.active());
        let id = runtime.selected.clone().unwrap();
        let ended = tokio::time::timeout(std::time::Duration::from_secs(15), async {
            let mut status = network.subscribe();
            loop {
                let current = status.borrow_and_update().clone();
                if current
                    .pairings
                    .iter()
                    .any(|p| p.id == id && p.stage == PairingStage::Failed)
                {
                    return current;
                }
                status.changed().await.unwrap();
            }
        })
        .await
        .expect("pairing with a closed port did not fail");
        scripted.status.send_replace(ended.clone());
        runtime.observe_selected();
        assert!(!runtime.active());
        // The network later evicts finished records; the outcome is kept.
        let mut forgotten = ended;
        forgotten.pairings.clear();
        scripted.status.send_replace(forgotten);
        assert!(
            !runtime.active(),
            "a forgotten ended pairing blocked new ones"
        );
        owner.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn periodic_and_network_driven_refreshes_do_not_republish_unchanged_state() {
        let (owner, network, _events) = crate::tests::start_network(Options {
            listen: "127.0.0.1:0".parse().unwrap(),
            pairing_listen: "127.0.0.1:0".parse().unwrap(),
            discovery: false,
            ..Default::default()
        })
        .await;
        let (_stop, stopped) = watch::channel(false);
        let (config, _config_runtime) = configuration::channel(
            Configuration::new(Settings {
                discoverable: false,
                ..Settings::default()
            }),
            stopped,
        );
        let (channels, mut runtime) = create(
            crate::features::devices::adapters::network::port(network),
            Arc::new(config),
        );
        let mut state = channels.state;
        runtime.reconcile().await.unwrap();
        runtime.refresh_addresses().await;
        runtime.publish();
        assert_ne!(state.borrow_and_update().local.sync_port, 0);

        // The work performed by every 5 s tick and every network notification.
        runtime.reconcile().await.unwrap();
        runtime.refresh_addresses().await;
        runtime.refresh_peer_names().await.unwrap();
        runtime.publish();
        assert!(!state.has_changed().unwrap(), "unchanged state republished");

        runtime.fault(&Error::Busy);
        assert!(state.has_changed().unwrap(), "new fault was not published");
        state.borrow_and_update();
        runtime.fault(&Error::Busy);
        assert!(!state.has_changed().unwrap(), "recurring fault republished");
        owner.shutdown().await.unwrap();
    }
}
