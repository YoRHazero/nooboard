use super::*;
use nooboard_clipboard::Payload;
use nooboard_network::{ApplicationOutcome, NetworkEvent, ReceiveDecision};
impl Runtime {
    pub(super) async fn receive(&mut self, event: NetworkEvent) -> Result<()> {
        match event {
            NetworkEvent::IncomingOffer {
                id,
                transfer_id,
                peer,
                kind,
                files,
            } => {
                let config = self.config.current();
                let directory = config.settings.receive_directory.clone();
                let accept = config.settings.accepting()
                    && config.peers.contains_key(&peer)
                    && (kind == crate::ContentKind::Image || directory.is_some());
                self.state.send_modify(|s| {
                    s.operations.push_front(crate::sync::model::Operation {
                        incoming_peer: Some(peer.clone()),
                        id: transfer_id,
                        automatic: false,
                        bytes: 0,
                        names: files.iter().map(|f| f.name.clone()).collect(),
                        kind: Some(kind),
                        at_ms: crate::history::now_ms(),
                    });
                });
                self.network
                    .decide_incoming(
                        id,
                        if accept {
                            ReceiveDecision::Accept {
                                directory: if kind == crate::ContentKind::Files {
                                    directory
                                } else {
                                    None
                                },
                            }
                        } else {
                            ReceiveDecision::Reject
                        },
                    )
                    .await?;
            }
            NetworkEvent::ContentReady {
                id,
                transfer_id,
                peer,
                content,
            } => {
                let config = self.config.current();
                if !config.settings.accepting() || !config.peers.contains_key(&peer) {
                    self.network
                        .complete_incoming(id, ApplicationOutcome::Rejected)
                        .await?;
                    return Ok(());
                }
                let payload = match crate::sync::content::incoming(content) {
                    Ok(p) => p,
                    Err(e) => {
                        self.network
                            .complete_incoming(id, ApplicationOutcome::Failed)
                            .await?;
                        return Err(e);
                    }
                };
                let result = self.apply(payload.clone(), Some(peer.clone())).await;
                if result.is_err() && !matches!(payload, Payload::Text(_)) {
                    self.state.send_modify(|s| {
                        s.application_failures
                            .insert((transfer_id.clone(), peer.clone()));
                    });
                }
                let outcome = if result.is_ok() {
                    ApplicationOutcome::Applied
                } else {
                    ApplicationOutcome::Failed
                };
                // Reporting the actual application outcome must not depend on history persistence.
                let receipt = self.network.complete_incoming(id, outcome).await;
                if result.is_ok() {
                    let text = match &payload {
                        Payload::Text(t) => {
                            if let Err(e) =
                                self.history
                                    .record(t.clone(), Some(peer.clone()), &config.settings)
                            {
                                self.fault(&e);
                            }
                            t.clone()
                        }
                        _ => "Received content".into(),
                    };
                    if matches!(payload, Payload::Text(_)) {
                        self.activity(
                            crate::ActivityKind::Received,
                            &text,
                            Some(peer.clone()),
                            Some(transfer_id.clone()),
                            None,
                        );
                    }
                }
                result?;
                receipt?;
            }
            _ => return Err(Error::Internal),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configuration::{
        model::{Configuration, Peer},
        runtime as configuration,
    };
    use crate::history::runtime as history;
    use nooboard_network::{
        IdentityOptions, NetworkService, Options, SendRequest, TransferStage, TrustedPeer,
    };
    use nooboard_storage::{BackendConfig, SqliteOptions, StorageService};
    use std::time::Duration;

    #[tokio::test]
    async fn failed_history_write_cannot_turn_a_successful_application_into_failure() {
        let options = || Options {
            identity: IdentityOptions::Ephemeral,
            listen: "127.0.0.1:0".parse().unwrap(),
            pairing_listen: "127.0.0.1:0".parse().unwrap(),
            discovery: false,
            ..Default::default()
        };
        let (owner_a, a, _a_events) = NetworkService::start(options()).await.unwrap();
        let (owner_b, b, mut events) = NetworkService::start(options()).await.unwrap();
        let trusted = |network: &Network| {
            let status = network.status();
            TrustedPeer {
                identity: status.identity,
                addresses: vec![status.listen_address],
            }
        };
        a.trust_peer(trusted(&b)).await.unwrap();
        b.trust_peer(trusted(&a)).await.unwrap();
        let mut status = a.subscribe();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if status
                    .borrow_and_update()
                    .connections
                    .iter()
                    .any(|p| p.accepting)
                {
                    break;
                }
                status.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
        let (storage_owner, storage) = StorageService::start(
            BackendConfig::Sqlite(SqliteOptions::in_memory()),
            Default::default(),
        )
        .await
        .unwrap();
        storage_owner.shutdown().await.unwrap();
        let (stop, stopped) = watch::channel(false);
        let mut document = Configuration::new(crate::Settings::default());
        let peer = trusted(&a);
        document.peers.insert(
            peer.identity.id.clone(),
            Peer {
                trusted: peer,
                settings: crate::PeerSettings {
                    address: None,
                    auto_send: false,
                },
            },
        );
        let (config, _config_runtime) = configuration::channel(document, stopped.clone());
        let (history, history_runtime) = history::channel(stopped.clone());
        let mut history_state = history.state.clone();
        let history_task = tokio::spawn(history_runtime.run(storage, config.clone(), stopped));
        let clipboard = Arc::new(crate::tests::Clipboard::new());
        let (_channels, mut runtime) = super::super::create(
            clipboard.clone(),
            b.clone(),
            config,
            history,
            clipboard.read().await.unwrap(),
        );
        let id = a
            .send(SendRequest::text(
                vec![b.status().identity.id],
                "application survives storage failure",
            ))
            .await
            .unwrap();
        let event = tokio::time::timeout(Duration::from_secs(5), events.next_event())
            .await
            .unwrap()
            .unwrap();
        runtime.receive(event).await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if history_state.borrow_and_update().error.is_some() {
                    break;
                }
                history_state.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if status
                    .borrow_and_update()
                    .transfers
                    .iter()
                    .any(|row| row.id == id && row.stage == TransferStage::Applied)
                {
                    break;
                }
                status.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
        assert_eq!(
            clipboard.text().as_deref(),
            Some("application survives storage failure")
        );
        stop.send_replace(true);
        history_task.await.unwrap().unwrap();
        owner_a.shutdown().await.unwrap();
        owner_b.shutdown().await.unwrap();
    }
}
