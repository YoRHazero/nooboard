use super::Runtime;
use crate::{
    Error, Event, Mode, Result,
    app::{Command, Reply},
    devices::{MAX_PEERS, Peer},
    history,
    link::Listener,
};
use nooboard_network::Message;

impl Runtime {
    pub(super) async fn command(&mut self, command: Command) -> Result<Reply> {
        match command {
            Command::RefreshDiscovery => {
                self.refresh_discovery();
                Ok(Reply::Done)
            }
            Command::BeginPairing { address, expected } => {
                self.begin_pairing(address, expected)?;
                Ok(Reply::Done)
            }
            Command::AcceptPairing(id) => {
                self.accept_pairing(&id)?;
                Ok(Reply::Done)
            }
            Command::SubmitPairingCode { id, code } => {
                self.submit_pairing_code(&id, code)?;
                Ok(Reply::Done)
            }
            Command::DismissPairing(id) => {
                self.dismiss_pairing(&id)?;
                Ok(Reply::Done)
            }
            Command::Settings(settings) => {
                settings.validate()?;
                self.observe(self.clipboard.read().await?, true).await?;
                let listener = if settings.listen_address != self.config.settings.listen_address {
                    Some(
                        Listener::bind(
                            &settings.listen_address,
                            self.config.tls(&self.identity)?,
                            self.link_events.clone(),
                        )
                        .await?,
                    )
                } else {
                    None
                };
                let pairing_listener = if settings.pairing_listen_address
                    != self.config.settings.pairing_listen_address
                {
                    Some(
                        nooboard_network::pairing::Endpoint::bind(
                            &settings.pairing_listen_address,
                            self.pairing_contact(),
                            self.onboarding.sender.clone(),
                        )
                        .await?,
                    )
                } else {
                    None
                };
                let acceptance_changed = settings.accepting() != self.config.settings.accepting();
                let renamed = settings.device_name != self.config.settings.device_name;
                let mut config = self.config.clone();
                config.settings = settings;
                config.save(&self.store).await?;
                self.config = config;
                if self.config.settings.paused {
                    self.cancel_content(None, false);
                } else if !self.config.settings.receive {
                    self.cancel_content(None, true);
                }
                if let Some(listener) = listener {
                    self.listener = listener;
                }
                if let Some(endpoint) = pairing_listener {
                    if let Some(session) = self.onboarding.snapshot.session.clone() {
                        self.dismiss_pairing(&session.id)?;
                    }
                    self.onboarding.snapshot.pairing_address = endpoint.address.clone();
                    self.onboarding.endpoint = endpoint;
                }
                self.onboarding
                    .endpoint
                    .contact
                    .send_replace(self.pairing_contact());
                self.refresh_local_network();
                self.onboarding.snapshot.discovery_error = self
                    .advertise_discovery()
                    .err()
                    .map(|_| "局域网发现暂不可用，可以通过地址添加设备。".into());
                let ids = self.peers.keys().cloned().collect::<Vec<_>>();
                for id in ids {
                    if self.config.settings.paused {
                        self.cancel_queued(&id, false);
                    } else if self.config.settings.mode != Mode::Automatic {
                        self.cancel_queued(&id, true);
                    }
                    if renamed {
                        self.control(
                            &id,
                            Message::Device {
                                device_name: self.config.settings.device_name.clone(),
                            },
                        );
                    }
                }
                if acceptance_changed {
                    self.advertise(true)?;
                }
                self.publish();
                if let Err(e) = history::prune(&self.store, &self.config.settings).await {
                    self.fault(None, e.to_string());
                }
                self.emit(Event::HistoryChanged);
                Ok(Reply::Done)
            }
            Command::TrustPeer(request) => {
                let mut peer = Peer::confirmed(request, &self.identity)?;
                if let Some(old) = self.config.peers.get(&peer.noob_id) {
                    if old.certificate != peer.certificate {
                        return Err(Error::AlreadyPaired);
                    }
                    peer.settings.auto_send = old.settings.auto_send;
                } else if self.config.peers.len() >= MAX_PEERS {
                    return Err(Error::Busy);
                }
                let id = peer.noob_id.clone();
                let mut config = self.config.clone();
                config.peers.insert(id.clone(), peer);
                let tls = config.tls(&self.identity)?;
                config.save(&self.store).await?;
                self.config = config;
                self.listener.trust.send_replace(tls);
                self.start_dial(&id)?;
                self.publish();
                Ok(Reply::Done)
            }
            Command::Unpair(id) => {
                let mut config = self.config.clone();
                if config.peers.remove(&id).is_none() {
                    return Err(Error::Configuration);
                }
                config.manual_targets.retain(|target| *target != id);
                let tls = config.tls(&self.identity)?;
                config.save(&self.store).await?;
                self.config = config;
                self.listener.trust.send_replace(tls);
                self.disconnect(&id);
                self.peers.remove(&id);
                self.received.forget(&id);
                self.publish();
                Ok(Reply::Done)
            }
            Command::ConfigurePeer(id, settings) => {
                settings.validate()?;
                self.observe(self.clipboard.read().await?, true).await?;
                let mut config = self.config.clone();
                let peer = config.peers.get_mut(&id).ok_or(Error::Configuration)?;
                let address_changed = peer.settings.address != settings.address;
                peer.settings = settings;
                config.save(&self.store).await?;
                self.config = config;
                if !self.config.peers[&id].settings.auto_send {
                    self.cancel_queued(&id, true);
                }
                if address_changed {
                    self.start_dial(&id)?;
                }
                self.publish();
                Ok(Reply::Done)
            }
            Command::SelectTargets(targets) => {
                self.select_targets(targets).await?;
                Ok(Reply::Done)
            }
            Command::Send(targets) => {
                if self.config.settings.paused {
                    return Err(Error::Paused);
                }
                if matches!(
                    self.view.snapshot.current.kind,
                    crate::ClipboardKind::Image | crate::ClipboardKind::Files
                ) {
                    if let Some(targets) = targets {
                        self.select_targets(targets).await?;
                    }
                    let id =
                        self.start_content(crate::content_transfer::workers::Source::Clipboard(
                            self.view.snapshot.current.revision,
                        ))?;
                    return Ok(Reply::MessageId(id));
                }
                let snapshot = self.clipboard.read().await?;
                self.observe(snapshot.clone(), false).await?;
                let text = Self::eligible(snapshot.content)?;
                if let Some(targets) = targets {
                    self.select_targets(targets).await?;
                }
                Ok(Reply::MessageId(self.send_text(
                    text,
                    self.config.manual_targets.clone(),
                    false,
                )?))
            }
            Command::History {
                contains,
                limit,
                offset,
                local,
            } => Ok(Reply::History(
                history::query(
                    &self.store,
                    &self.config.settings,
                    contains,
                    local,
                    limit,
                    offset,
                )
                .await?,
            )),
            Command::SendFiles(paths) => {
                self.start_content(crate::content_transfer::workers::Source::Files(paths))?;
                Ok(Reply::Done)
            }
            Command::CancelContent(key) => {
                if self.content.cancel(&key) {
                    if self.content.row(&key).is_some_and(|r| !r.stage.pending()) {
                        self.content_node(&key, "finished");
                    }
                    self.publish_snapshot();
                }
                Ok(Reply::Done)
            }
            Command::CopyContent(key) => {
                self.copy_content(&key)?;
                Ok(Reply::Done)
            }
            Command::CopyHistory(id) => {
                history::prune(&self.store, &self.config.settings).await?;
                let entry = self
                    .store
                    .run(move |db| db.history_entry(id))
                    .await?
                    .ok_or(Error::NotFound)?;
                let snapshot = self.clipboard.write(entry.text.clone()).await?;
                self.observed_revision = snapshot.revision;
                self.view.current(snapshot.clone(), None);
                self.view
                    .record(crate::ActivityKind::Copied, &entry.text, None, None, None);
                self.remember(entry.text.clone(), "local".into()).await;
                self.emit(Event::Copied {
                    revision: snapshot.revision,
                    bytes: entry.text.len(),
                });
                self.auto_send(entry.text)?;
                Ok(Reply::Done)
            }
            Command::DeleteHistory(id) => {
                self.store.run(move |db| db.delete_history(id)).await?;
                self.emit(Event::HistoryChanged);
                Ok(Reply::Done)
            }
            Command::ClearHistory => {
                self.store.run(|db| db.clear_history()).await?;
                self.emit(Event::HistoryChanged);
                Ok(Reply::Done)
            }
            Command::Stop => Ok(Reply::Done),
        }
    }
    async fn select_targets(&mut self, targets: Vec<String>) -> Result<()> {
        self.config.targets(&targets)?;
        let mut config = self.config.clone();
        config.manual_targets = targets;
        config.save(&self.store).await?;
        self.config = config;
        self.publish();
        Ok(())
    }
}
