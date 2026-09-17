use super::{PeerSession, Runtime};
use crate::{
    DeliveryState, Event, Result,
    link::{Dial, LinkEvent, Session},
};
use nooboard_network::{Message, TlsConfig};

impl Runtime {
    pub(super) fn start_dial(&mut self, id: &str) -> Result<()> {
        let generation = self.fresh_generation()?;
        let peer = self
            .config
            .peers
            .get(id)
            .ok_or(crate::Error::Configuration)?;
        let state = self
            .peers
            .entry(id.to_owned())
            .or_insert_with(PeerSession::new);
        state.dial = None;
        state.dial_generation = generation;
        let mut addresses = peer.settings.address.iter().cloned().collect::<Vec<_>>();
        for device in &self.onboarding.snapshot.nearby {
            if device.noob_id == id {
                for address in &device.addresses {
                    if let Ok(mut address) = address.parse::<std::net::SocketAddr>() {
                        address.set_port(device.sync_port);
                        let address = address.to_string();
                        if !addresses.contains(&address) {
                            addresses.push(address);
                        }
                    }
                }
            }
        }
        addresses.truncate(17);
        if !addresses.is_empty() {
            state.dial = Some(Dial::start(
                id.to_owned(),
                addresses,
                generation,
                TlsConfig::new(&self.identity, &peer.certificate)?,
                state.online.subscribe(),
                self.link_events.clone(),
            ));
        }
        Ok(())
    }
    pub(super) fn disconnect(&mut self, peer: &str) {
        self.disconnect_content(peer);
        if let Some(state) = self.peers.get_mut(peer) {
            state.session = None;
            state.peer_epoch = None;
            state.accepting = false;
            state.online.send_replace(false);
        }
        for transfer in self.transfers.disconnect(peer) {
            self.emit(Event::Transfer(transfer));
        }
        self.publish();
    }
    pub(super) fn control(&mut self, peer: &str, message: Message) {
        let failed = self
            .peers
            .get(peer)
            .and_then(|s| s.session.as_ref())
            .is_some_and(|s| s.outbox.control(message).is_err());
        if failed {
            self.disconnect(peer);
        }
    }
    pub(super) fn cancel_queued(&mut self, peer: &str, automatic_only: bool) {
        let ids = self
            .peers
            .get(peer)
            .and_then(|s| s.session.as_ref())
            .map(|s| s.outbox.cancel_text(automatic_only))
            .unwrap_or_default();
        for id in ids {
            self.delivery(&id, peer, DeliveryState::Cancelled);
        }
    }
    pub(super) async fn link_event(&mut self, event: LinkEvent) -> Result<()> {
        match event {
            LinkEvent::Connected {
                connection,
                initiated,
                dial_generation,
            } => {
                let id = connection.peer_id().to_owned();
                let Some(peer) = self.config.peers.get(&id) else {
                    return Ok(());
                };
                if nooboard_network::fingerprint(&peer.certificate) != connection.peer_fingerprint()
                {
                    return Ok(());
                }
                let preferred = initiated == (self.state.noob_id < id);
                if let Some(state) = self.peers.get(&id) {
                    if dial_generation.is_some_and(|g| g != state.dial_generation) {
                        return Ok(());
                    }
                    if state.session.is_some() && (state.preferred || !preferred) {
                        return Ok(());
                    }
                }
                // Existing online peers may see this local copy; the new peer must not replay it.
                self.observe(self.clipboard.read().await?, true).await?;
                self.disconnect(&id);
                let generation = self.fresh_generation()?;
                let session =
                    Session::start(connection, id.clone(), generation, self.link_events.clone());
                let state = self
                    .peers
                    .entry(id.clone())
                    .or_insert_with(PeerSession::new);
                state.generation = generation;
                state.local_epoch = generation;
                state.preferred = preferred;
                state.session = Some(session);
                state.online.send_replace(true);
                self.control(
                    &id,
                    Message::Device {
                        device_name: self.config.settings.device_name.clone(),
                    },
                );
                self.control(
                    &id,
                    Message::State {
                        epoch: generation,
                        accepting: self.config.settings.accepting(),
                    },
                );
                self.publish();
            }
            LinkEvent::Message {
                peer,
                generation,
                message,
            } if self.current(&peer, generation) => {
                if let Err(error) = self.receive(&peer, message).await {
                    self.fault(Some(peer.clone()), error.to_string());
                    self.disconnect(&peer);
                }
            }
            LinkEvent::Started {
                peer,
                generation,
                id,
            } if self.current(&peer, generation) => {
                self.delivery(&id, &peer, DeliveryState::Sending)
            }
            LinkEvent::Written {
                peer,
                generation,
                id,
            } if self.current(&peer, generation) => {
                self.delivery(&id, &peer, DeliveryState::AwaitingReceipt)
            }
            LinkEvent::Offline { peer, generation } if self.current(&peer, generation) => {
                self.disconnect(&peer)
            }
            LinkEvent::Fault { peer, message } => self.fault(peer, message),
            _ => {}
        }
        Ok(())
    }
    fn current(&self, peer: &str, generation: u64) -> bool {
        self.peers
            .get(peer)
            .is_some_and(|s| s.session.is_some() && s.generation == generation)
    }
    pub(super) fn advertise(&mut self, new_epoch: bool) -> Result<()> {
        for id in self.peers.keys().cloned().collect::<Vec<_>>() {
            if new_epoch {
                let epoch = self.fresh_generation()?;
                self.peers.get_mut(&id).expect("peer exists").local_epoch = epoch;
            }
            let epoch = self.peers[&id].local_epoch;
            self.control(
                &id,
                Message::State {
                    epoch,
                    accepting: self.config.settings.accepting(),
                },
            );
        }
        Ok(())
    }
}
