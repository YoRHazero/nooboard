use super::*;
use nooboard_clipboard::{Payload, ReadState};
impl Runtime {
    pub(super) async fn send_current(&mut self, targets: Option<Vec<String>>) -> Result<MessageId> {
        if let Some(targets) = targets {
            self.config
                .change(crate::configuration::runtime::Change::Targets(targets))
                .await?;
        }
        let snapshot = self.clipboard.read().await?;
        self.observe(snapshot.clone(), false).await?;
        let ReadState::Ready(payload) = snapshot.content else {
            return Err(Error::Ineligible);
        };
        self.send_payload(payload, self.config.current().manual_targets, false)
            .await
    }
    pub(super) async fn send_payload(
        &mut self,
        payload: Payload,
        targets: Vec<String>,
        automatic: bool,
    ) -> Result<MessageId> {
        let config = self.config.current();
        if config.settings.paused {
            return Err(Error::Paused);
        }
        if targets.is_empty() {
            return Err(Error::NoTargets);
        }
        config.targets(&targets)?;
        let (kind, names, bytes) = match &payload {
            Payload::Text(t) => (None, vec![], t.len()),
            Payload::Image(image) => (
                Some(crate::ContentKind::Image),
                vec!["image.png".into()],
                image.bytes().len(),
            ),
            Payload::Files(paths) => (
                Some(crate::ContentKind::Files),
                paths
                    .iter()
                    .map(|p| {
                        p.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned()
                    })
                    .collect(),
                0,
            ),
        };
        let summary = match &payload {
            Payload::Text(t) => t.clone(),
            _ => names.join(", "),
        };
        let content = crate::sync::content::outgoing(payload).await?;
        // Recheck policy after potentially expensive image conversion.
        if self.config.current().settings.paused {
            return Err(Error::Paused);
        }
        let id = self
            .network
            .send(nooboard_network::SendRequest {
                targets: targets.clone(),
                content,
                queue: if automatic {
                    nooboard_network::QueuePolicy::ReplaceTail("automatic-text".into())
                } else {
                    nooboard_network::QueuePolicy::Append
                },
            })
            .await?;
        self.state.send_modify(|s| {
            s.operations.push_front(crate::sync::model::Operation {
                incoming_peer: None,
                id: id.clone(),
                automatic,
                bytes,
                names,
                kind,
                at_ms: crate::history::now_ms(),
            });
            let network = self.network.status();
            let mut finished = 0;
            s.operations.retain(|op| {
                if network
                    .transfers
                    .iter()
                    .any(|r| op.matches(r) && !r.stage.is_terminal())
                {
                    true
                } else {
                    finished += 1;
                    finished <= 64
                }
            });
        });
        if kind.is_none() {
            self.activity(
                crate::ActivityKind::Sent,
                &summary,
                None,
                Some(id.clone()),
                None,
            );
        }
        Ok(id)
    }
}
