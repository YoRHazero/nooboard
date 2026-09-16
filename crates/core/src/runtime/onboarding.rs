use super::Runtime;
use crate::onboarding::PairingFailure;
use crate::{
    Error, PairingSession, PeerSettings, Result, VerifiedPeer, app::Command, history::now_ms,
};
use nooboard_network::pairing::{Contact, Event, Stage};

impl Runtime {
    pub(super) fn refresh_discovery(&mut self) {
        self.refresh_local_network();
        let result = (|| -> std::result::Result<(), String> {
            if self.onboarding.discovery.is_none() {
                let discovery =
                    nooboard_network::discovery::Discovery::new(self.state.noob_id.clone())?;
                self.onboarding.nearby = discovery.subscribe();
                self.onboarding.discovery = Some(discovery);
            }
            self.onboarding
                .discovery
                .as_mut()
                .expect("discovery")
                .refresh()?;
            self.advertise_discovery()
        })();
        self.onboarding.snapshot.discovery_error = result
            .err()
            .map(|_| "局域网发现暂不可用，可以通过地址添加设备。".into());
        self.publish_snapshot();
    }
    pub(super) fn advertise_discovery(&mut self) -> std::result::Result<(), String> {
        if let Some(discovery) = &mut self.onboarding.discovery {
            discovery.advertise(
                &self.config.settings.device_name,
                self.onboarding
                    .endpoint
                    .address
                    .parse()
                    .map_err(|_| "配对地址无效")?,
                self.listener
                    .address
                    .parse::<std::net::SocketAddr>()
                    .map_err(|_| "监听地址无效")?
                    .port(),
                self.config.settings.discoverable,
            )?;
        }
        Ok(())
    }
    pub(super) fn pairing_contact(&self) -> Contact {
        Contact {
            device_name: self.config.settings.device_name.clone(),
            certificate: self.identity.certificate().to_vec(),
            sync_port: self
                .listener
                .address
                .parse::<std::net::SocketAddr>()
                .expect("bound address")
                .port(),
        }
    }
    pub(super) fn begin_pairing(
        &mut self,
        address: String,
        expected: Option<String>,
    ) -> Result<()> {
        if self
            .onboarding
            .snapshot
            .session
            .as_ref()
            .is_some_and(|s| s.active())
        {
            return Err(Error::Busy);
        }
        PeerSettings {
            address: Some(address.clone()),
            auto_send: false,
        }
        .validate()?;
        if expected.as_ref().is_some_and(|id| {
            id.len() != 64
                || !id.bytes().all(|b| b.is_ascii_hexdigit())
                || *id == self.state.noob_id
        }) {
            return Err(Error::Configuration);
        }
        let mut addresses = vec![address.clone()];
        if let Some(expected) = &expected {
            for device in &self.onboarding.snapshot.nearby {
                if &device.noob_id == expected {
                    for candidate in &device.addresses {
                        if !addresses.contains(candidate) {
                            addresses.push(candidate.clone());
                        }
                    }
                }
            }
        }
        addresses.truncate(16);
        let id = nooboard_network::new_session_id()?;
        let control = self
            .onboarding
            .endpoint
            .connect(id.clone(), addresses)
            .map_err(|_| Error::Busy)?;
        self.onboarding.control = Some(control);
        self.onboarding.expected = expected;
        self.onboarding.snapshot.session = Some(PairingSession {
            id,
            incoming: false,
            device_name: address,
            noob_id: None,
            stage: Stage::Requesting,
            code: None,
            expires_at_ms: now_ms() + 60_000,
            attempts_left: 3,
            error: None,
        });
        self.publish_snapshot();
        Ok(())
    }
    pub(super) fn accept_pairing(&mut self, id: &str) -> Result<()> {
        let session = self
            .onboarding
            .snapshot
            .session
            .as_ref()
            .filter(|s| s.id == id && s.incoming && s.stage == Stage::AwaitingApproval)
            .ok_or(Error::Configuration)?;
        let _ = session;
        self.onboarding
            .control
            .as_ref()
            .ok_or(Error::Stopped)?
            .accept()
            .map_err(|_| Error::Busy)
    }
    pub(super) fn submit_pairing_code(&mut self, id: &str, code: String) -> Result<()> {
        let session = self
            .onboarding
            .snapshot
            .session
            .as_mut()
            .filter(|s| s.id == id && !s.incoming && s.stage == Stage::EnteringCode)
            .ok_or(Error::Configuration)?;
        self.onboarding
            .control
            .as_ref()
            .ok_or(Error::Stopped)?
            .code(code)
            .map_err(|_| Error::Configuration)?;
        session.stage = Stage::Verifying;
        session.error = None;
        self.publish_snapshot();
        Ok(())
    }
    pub(super) fn dismiss_pairing(&mut self, id: &str) -> Result<()> {
        if self
            .onboarding
            .snapshot
            .session
            .as_ref()
            .is_none_or(|s| s.id != id)
        {
            return Ok(());
        }
        if let Some(control) = self.onboarding.control.take() {
            control.cancel();
        }
        self.onboarding.snapshot.session = None;
        self.onboarding.expected = None;
        self.publish_snapshot();
        Ok(())
    }
    pub(super) async fn pairing_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Offered {
                id,
                peer,
                control,
                incoming,
            } => {
                let Ok(noob_id) = peer.validate() else {
                    control.cancel();
                    return Ok(());
                };
                if incoming {
                    if self
                        .onboarding
                        .snapshot
                        .session
                        .as_ref()
                        .is_some_and(|s| s.active())
                    {
                        control.cancel();
                        return Ok(());
                    }
                    self.onboarding.snapshot.session = Some(PairingSession {
                        id,
                        incoming: true,
                        device_name: peer.device_name,
                        noob_id: Some(noob_id),
                        stage: Stage::AwaitingApproval,
                        code: None,
                        expires_at_ms: now_ms() + 60_000,
                        attempts_left: 3,
                        error: None,
                    });
                    self.onboarding.control = Some(control);
                    self.onboarding.expected = None;
                } else if let Some(session) = self
                    .onboarding
                    .snapshot
                    .session
                    .as_mut()
                    .filter(|s| s.id == id && s.active())
                {
                    if self
                        .onboarding
                        .expected
                        .as_ref()
                        .is_some_and(|expected| expected != &noob_id)
                    {
                        control.cancel();
                        session.stage = Stage::Failed;
                        session.error = Some(PairingFailure::IdentityChanged);
                    } else {
                        session.device_name = peer.device_name;
                        session.noob_id = Some(noob_id);
                    }
                } else {
                    control.cancel();
                }
            }
            Event::Progress {
                id,
                stage,
                code,
                attempts_left,
            } => {
                if let Some(session) = self
                    .onboarding
                    .snapshot
                    .session
                    .as_mut()
                    .filter(|s| s.id == id && s.active())
                {
                    if matches!(stage, Stage::ShowingCode | Stage::EnteringCode)
                        && matches!(session.stage, Stage::Requesting | Stage::AwaitingApproval)
                    {
                        session.expires_at_ms = now_ms() + 120_000;
                    }
                    session.error = if stage == Stage::EnteringCode && attempts_left < 3 {
                        Some(PairingFailure::IncorrectCode {
                            remaining: attempts_left,
                        })
                    } else {
                        None
                    };
                    session.stage = stage;
                    session.code = code;
                    session.attempts_left = attempts_left;
                }
            }
            Event::Verified {
                id,
                peer,
                address,
                saved,
            } => {
                if self
                    .onboarding
                    .snapshot
                    .session
                    .as_ref()
                    .is_none_or(|s| s.id != id || s.stage != Stage::Saving)
                {
                    let _ = saved.send(false);
                    return Ok(());
                }
                let request = VerifiedPeer {
                    confirmed_fingerprint: nooboard_network::fingerprint(&peer.certificate),
                    certificate: peer.certificate,
                    device_name: peer.device_name,
                    address: Some(address),
                };
                let result = self.command(Command::TrustPeer(request)).await;
                let _ = saved.send(result.is_ok());
                if result.is_err()
                    && let Some(session) = &mut self.onboarding.snapshot.session
                {
                    session.error = Some(PairingFailure::Storage);
                }
            }
            Event::Completed { id } => {
                if let Some(session) = self
                    .onboarding
                    .snapshot
                    .session
                    .as_mut()
                    .filter(|s| s.id == id && s.active())
                {
                    session.stage = Stage::Completed;
                    session.code = None;
                    session.error = None;
                }
            }
            Event::Failed { id, error } => {
                if let Some(session) = self
                    .onboarding
                    .snapshot
                    .session
                    .as_mut()
                    .filter(|s| s.id == id && s.active())
                {
                    session.stage = Stage::Failed;
                    session.code = None;
                    session.error = Some(PairingFailure::Network(error));
                }
            }
        }
        self.publish_snapshot();
        Ok(())
    }
}
