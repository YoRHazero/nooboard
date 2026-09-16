use super::Runtime;
use crate::{
    Delivery, DeliveryState, Error, Event, Mode, Result, Transfer, history, link::queue::Job,
};
use nooboard_clipboard::{Content, Origin, Snapshot};
use nooboard_network::{MAX_TEXT_BYTES, Message, MessageId};

impl Runtime {
    pub(super) fn eligible(content: Content) -> Result<String> {
        match content {
            Content::Text(text) if text.len() <= MAX_TEXT_BYTES && !text.contains('\0') => Ok(text),
            _ => Err(Error::Ineligible),
        }
    }
    pub(super) async fn remember(&mut self, text: String, source: String) {
        if self.config.settings.history {
            match history::record(&self.store, &self.config.settings, text, source).await {
                Ok(()) => self.emit(Event::HistoryChanged),
                Err(e) => self.fault(None, e.to_string()),
            }
        }
    }
    pub(super) async fn observe(&mut self, snapshot: Snapshot, allow_send: bool) -> Result<()> {
        if snapshot.revision == self.observed_revision {
            return Ok(());
        }
        self.observed_revision = snapshot.revision;
        self.view.current(snapshot.clone(), None);
        if snapshot.origin == Origin::Application {
            self.publish_snapshot();
            return Ok(());
        }
        let Ok(text) = Self::eligible(snapshot.content) else {
            self.publish_snapshot();
            return Ok(());
        };
        self.view
            .record(crate::ActivityKind::Copied, &text, None, None, None);
        self.remember(text.clone(), "local".into()).await;
        self.emit(Event::Copied {
            revision: snapshot.revision,
            bytes: text.len(),
        });
        if allow_send {
            self.auto_send(text)?;
        }
        Ok(())
    }
    pub(super) fn auto_send(&mut self, text: String) -> Result<()> {
        if self.config.settings.mode != Mode::Automatic || self.config.settings.paused {
            return Ok(());
        }
        let targets = self
            .config
            .peers
            .values()
            .filter(|p| p.settings.auto_send && self.ready(&p.noob_id))
            .map(|p| p.noob_id.clone())
            .collect::<Vec<_>>();
        if !targets.is_empty() {
            self.send_text(text, targets, true)?;
        }
        Ok(())
    }
    fn ready(&self, peer: &str) -> bool {
        self.peers
            .get(peer)
            .is_some_and(|s| s.session.is_some() && s.accepting && s.peer_epoch.is_some())
    }
    pub(super) fn send_text(
        &mut self,
        text: String,
        targets: Vec<String>,
        automatic: bool,
    ) -> Result<MessageId> {
        if self.config.settings.paused {
            return Err(Error::Paused);
        }
        if targets.is_empty() {
            return Err(Error::NoTargets);
        }
        self.config.targets(&targets)?;
        if !self.transfers.available() {
            return Err(Error::Busy);
        }
        self.sequence = self.sequence.checked_add(1).ok_or(Error::Configuration)?;
        let id = MessageId {
            session: self.namespace.clone(),
            sequence: self.sequence,
        };
        let mut transfer = Transfer {
            id: id.clone(),
            automatic,
            bytes: text.len(),
            targets: Vec::new(),
        };
        let mut superseded = Vec::new();
        for peer in targets {
            let mut outcome = DeliveryState::Offline;
            if self.ready(&peer) {
                let state = &self.peers[&peer];
                let job = Job {
                    automatic,
                    message: Message::Text {
                        id: id.clone(),
                        target_epoch: state.peer_epoch.expect("ready epoch"),
                        text: text.clone(),
                    },
                };
                match state
                    .session
                    .as_ref()
                    .expect("ready session")
                    .outbox
                    .text(job)
                {
                    Ok(old) => {
                        outcome = DeliveryState::Queued;
                        if let Some(old) = old {
                            superseded.push((peer.clone(), old));
                        }
                    }
                    Err(()) => outcome = DeliveryState::QueueFull,
                }
            }
            transfer.targets.push(Delivery {
                noob_id: peer.clone(),
                device_name: self.config.peers[&peer].device_name.clone(),
                state: outcome,
            });
        }
        self.view.record(
            crate::ActivityKind::Sent,
            &text,
            None,
            None,
            Some(id.clone()),
        );
        self.transfers.insert(transfer.clone());
        self.emit(Event::Transfer(transfer));
        for (peer, old) in superseded {
            self.delivery(&old, &peer, DeliveryState::Superseded);
        }
        self.publish();
        Ok(id)
    }
    pub(super) async fn receive(&mut self, peer: &str, message: Message) -> Result<()> {
        match message {
            Message::Device { device_name } => {
                if self.config.peers[peer].device_name != device_name {
                    let mut config = self.config.clone();
                    config
                        .peers
                        .get_mut(peer)
                        .expect("authenticated peer")
                        .device_name = device_name;
                    config.save(&self.store).await?;
                    self.config = config;
                    self.publish();
                }
            }
            Message::State { epoch, accepting } => {
                if self.peers[peer]
                    .peer_epoch
                    .is_some_and(|previous| epoch <= previous)
                {
                    return Err(Error::Configuration);
                }
                // Copies preceding readiness must not be replayed to this newly ready peer.
                self.observe(self.clipboard.read().await?, true).await?;
                self.cancel_queued(peer, false);
                let state = self.peers.get_mut(peer).expect("authenticated peer");
                state.peer_epoch = Some(epoch);
                state.accepting = accepting;
                self.publish();
            }
            Message::Text {
                id,
                target_epoch,
                text,
            } => {
                if let Some(applied) = self.received.outcome(peer, &id) {
                    self.control(
                        peer,
                        if applied {
                            Message::Applied { id }
                        } else {
                            Message::Rejected { id }
                        },
                    );
                    return Ok(());
                }
                if !self.config.settings.accepting() || target_epoch != self.peers[peer].local_epoch
                {
                    self.received.record(peer, &id, false);
                    self.control(peer, Message::Rejected { id });
                    return Ok(());
                }
                // Capture a real local copy before overwriting it; remote writes never enter fan-out.
                self.observe(self.clipboard.read().await?, true).await?;
                match self.clipboard.write(text.clone()).await {
                    Ok(snapshot) => {
                        self.observed_revision = snapshot.revision;
                        self.view.current(snapshot, Some(peer.to_owned()));
                        self.view.record(
                            crate::ActivityKind::Received,
                            &text,
                            Some(peer.to_owned()),
                            Some(self.config.peers[peer].device_name.clone()),
                            Some(id.clone()),
                        );
                        self.received.record(peer, &id, true);
                        self.emit(Event::Received {
                            source: peer.to_owned(),
                            device_name: self.config.peers[peer].device_name.clone(),
                            id: id.clone(),
                            bytes: text.len(),
                        });
                        self.control(peer, Message::Applied { id });
                        self.remember(text, peer.to_owned()).await;
                    }
                    Err(error) => {
                        self.received.record(peer, &id, false);
                        self.control(peer, Message::Rejected { id });
                        self.fault(Some(peer.to_owned()), error.to_string());
                    }
                }
            }
            Message::Applied { id } => self.delivery(&id, peer, DeliveryState::Applied),
            Message::Rejected { id } => self.delivery(&id, peer, DeliveryState::Rejected),
            _ => return Err(Error::Configuration),
        }
        Ok(())
    }
}
