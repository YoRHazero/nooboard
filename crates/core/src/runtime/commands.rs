use super::Runtime;
use crate::{
    Error, Event, Result,
    app::{Command, Reply},
    history,
    model::Peer,
};
use nooboard_network::{Message, TlsConfig, fingerprint};

impl Runtime {
    pub(super) async fn command(&mut self, command: Command) -> Result<Reply> {
        match command {
            Command::Settings(settings) => {
                settings.validate()?;
                // Consume pre-transition content under the previous mode to avoid replay on resume.
                let current = self.clipboard.read().await?;
                self.observe(current, false).await?;
                let bytes = serde_json::to_vec(&settings).map_err(|_| Error::Configuration)?;
                self.store
                    .run(move |db| db.set_setting("settings", &bytes))
                    .await?;
                let acceptance_changed = settings.accepting() != self.state.settings.accepting();
                self.state.settings = settings;
                if acceptance_changed {
                    self.epoch = self.epoch.checked_add(1).ok_or(Error::Configuration)?;
                }
                self.publish();
                if acceptance_changed
                    && self.connection.is_some()
                    && let Err(error) = self
                        .send(Message::State {
                            epoch: self.epoch,
                            accepting: self.state.settings.accepting(),
                        })
                        .await
                {
                    self.emit(Event::Fault(error.to_string()));
                }
                if let Err(error) = history::prune(&self.store, &self.state.settings).await {
                    self.emit(Event::Fault(error.to_string()));
                }
                Ok(Reply::Done)
            }
            Command::Pair(request) => {
                if request.certificate.len() > 8192
                    || fingerprint(&request.certificate)
                        != request.confirmed_fingerprint.to_lowercase()
                {
                    return Err(Error::Fingerprint);
                }
                if request.certificate == self.identity.certificate() {
                    return Err(Error::Configuration);
                }
                if self
                    .peer
                    .as_ref()
                    .is_some_and(|p| p.certificate != request.certificate)
                {
                    return Err(Error::AlreadyPaired);
                }
                TlsConfig::new(&self.identity, &request.certificate)?;
                let peer = Peer {
                    certificate: request.certificate,
                    endpoint: request.endpoint,
                };
                let bytes = serde_json::to_vec(&peer).map_err(|_| Error::Configuration)?;
                self.store
                    .run(move |db| db.set_setting("peer", &bytes))
                    .await?;
                self.state.peer_fingerprint = Some(fingerprint(&peer.certificate));
                self.peer = Some(peer);
                self.restart_link()?;
                Ok(Reply::Done)
            }
            Command::Unpair => {
                self.store.run(|db| db.delete_setting("peer")).await?;
                self.peer = None;
                self.state.peer_fingerprint = None;
                self.restart_link()?;
                Ok(Reply::Done)
            }
            Command::Send => {
                let snapshot = self.clipboard.read().await?;
                self.observe(snapshot.clone(), false).await?;
                let text = Self::eligible(snapshot.content)?;
                Ok(Reply::Sequence(self.send_text(text).await?))
            }
            Command::History {
                contains,
                limit,
                offset,
            } => Ok(Reply::History(
                history::query(&self.store, &self.state.settings, contains, limit, offset).await?,
            )),
            Command::CopyHistory(id) => {
                history::prune(&self.store, &self.state.settings).await?;
                let entry = self
                    .store
                    .run(move |db| db.history_entry(id))
                    .await?
                    .ok_or(Error::NotFound)?;
                let snapshot = self.clipboard.write(entry.text.clone()).await?;
                self.observed_revision = snapshot.revision;
                self.remember(entry.text.clone(), "local".into()).await;
                if self.can_auto_send() {
                    self.send_text(entry.text).await?;
                }
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
}
