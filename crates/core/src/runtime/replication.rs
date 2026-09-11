use super::Runtime;
use crate::{Error, Event, Mode, Result, history, link::LinkEvent, sync::Order};
use nooboard_clipboard::{Content, Origin, Snapshot};
use nooboard_network::{MAX_TEXT_BYTES, Message};

impl Runtime {
    pub(super) fn eligible(content: Content) -> Result<String> {
        match content {
            Content::Text(text) if text.len() <= MAX_TEXT_BYTES && !text.contains('\0') => Ok(text),
            _ => Err(Error::Ineligible),
        }
    }
    pub(super) fn can_auto_send(&self) -> bool {
        self.state.settings.mode == Mode::Automatic
            && !self.state.settings.paused
            && self.state.online
            && self.state.peer_accepting
    }
    pub(super) async fn remember(&self, text: String, source: String) {
        if self.state.settings.history {
            match history::record(&self.store, &self.state.settings, text, source).await {
                Ok(()) => self.emit(Event::HistoryChanged),
                Err(error) => self.emit(Event::Fault(error.to_string())),
            }
        }
    }
    pub(super) async fn observe(&mut self, snapshot: Snapshot, allow_send: bool) -> Result<()> {
        if snapshot.revision == self.observed_revision {
            return Ok(());
        }
        self.observed_revision = snapshot.revision;
        if snapshot.origin == Origin::Application {
            return Ok(());
        }
        let Ok(text) = Self::eligible(snapshot.content) else {
            return Ok(());
        };
        self.remember(text.clone(), "local".into()).await;
        if allow_send && self.can_auto_send() {
            self.send_text(text).await?;
        }
        Ok(())
    }
    pub(super) async fn send_text(&mut self, text: String) -> Result<u64> {
        if self.state.settings.paused {
            return Err(Error::Paused);
        }
        if !self.state.peer_accepting {
            return Err(Error::Offline);
        }
        let target_epoch = self.peer_epoch.ok_or(Error::Offline)?;
        let sequence = self.order.local(&self.state.fingerprint)?;
        self.send(Message::Text {
            sequence,
            target_epoch,
            text,
        })
        .await?;
        self.emit(Event::Sent { sequence });
        Ok(sequence)
    }
    pub(super) async fn link_event(&mut self, event: LinkEvent) -> Result<()> {
        match event {
            LinkEvent::Online { generation, sender } if generation.0 == self.instance => {
                // Never publish an offline clipboard snapshot when a new connection opens.
                self.observe(self.clipboard.read().await?, false).await?;
                self.connection = Some((generation, sender));
                self.order = Order::default();
                self.peer_epoch = None;
                self.epoch = self.epoch.checked_add(1).ok_or(Error::Configuration)?;
                self.state.online = true;
                self.state.peer_accepting = false;
                self.send(Message::State {
                    epoch: self.epoch,
                    accepting: self.state.settings.accepting(),
                })
                .await?;
                self.publish();
            }
            LinkEvent::Offline { generation }
                if self.connection.as_ref().is_some_and(|c| c.0 == generation) =>
            {
                self.connection = None;
                self.peer_epoch = None;
                self.state.online = false;
                self.state.peer_accepting = false;
                self.publish();
            }
            LinkEvent::Message {
                generation,
                message,
            } if self.connection.as_ref().is_some_and(|c| c.0 == generation) => {
                self.receive(message).await?;
            }
            LinkEvent::Fault { instance, message } if instance == self.instance => {
                self.emit(Event::Fault(message))
            }
            _ => {}
        }
        Ok(())
    }
    async fn receive(&mut self, message: Message) -> Result<()> {
        match message {
            Message::State { epoch, accepting } => {
                if self.peer_epoch.is_some_and(|previous| epoch <= previous) {
                    return Err(Error::Configuration);
                }
                self.peer_epoch = Some(epoch);
                self.state.peer_accepting = accepting;
                self.publish();
            }
            Message::Text {
                sequence,
                target_epoch,
                text,
            } => {
                if !self.state.settings.accepting() || target_epoch != self.epoch {
                    self.send(Message::Rejected { sequence }).await?;
                    return Ok(());
                }
                // Observe a local copy before comparing the remote Lamport order.
                self.observe(self.clipboard.read().await?, true).await?;
                let peer = self.state.peer_fingerprint.clone().ok_or(Error::Offline)?;
                if !self.order.should_apply(sequence, &peer) {
                    self.send(Message::Rejected { sequence }).await?;
                    return Ok(());
                }
                match self.clipboard.write(text.clone()).await {
                    Ok(snapshot) => {
                        self.observed_revision = snapshot.revision;
                        self.order.applied(sequence, &peer);
                        self.remember(text.clone(), peer).await;
                        self.emit(Event::Received { bytes: text.len() });
                        self.send(Message::Applied { sequence }).await?;
                    }
                    Err(error) => {
                        self.send(Message::Rejected { sequence }).await?;
                        return Err(error.into());
                    }
                }
            }
            Message::Applied { sequence } => self.emit(Event::Applied { sequence }),
            Message::Rejected { sequence } => self.emit(Event::Rejected { sequence }),
            _ => return Err(Error::Configuration),
        }
        Ok(())
    }
}
