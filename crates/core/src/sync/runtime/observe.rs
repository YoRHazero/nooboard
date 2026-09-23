use super::*;
use nooboard_clipboard::{Origin, Payload, ReadState};
impl Runtime {
    pub(super) async fn observe(&mut self, snapshot: Snapshot, allow_send: bool) -> Result<()> {
        // A clipboard notification can win the select against a configuration or
        // connection notification. Establish every newly eligible destination's
        // baseline before applying the policy, regardless of notification order.
        let config = if allow_send {
            self.refresh_routes().await?
        } else {
            self.config.current()
        };
        if snapshot.revision <= self.observed {
            return Ok(());
        }
        self.observed = snapshot.revision;
        self.state.send_modify(|s| {
            s.current = crate::CurrentClipboard::from_native(snapshot.clone(), None)
        });
        if let ReadState::Ready(Payload::Image(image)) = &snapshot.content {
            self.pending_preview = Some((snapshot.revision, image.clone()));
            self.start_preview();
        }
        if snapshot.origin == Origin::Application {
            return Ok(());
        }
        if let ReadState::Ready(Payload::Text(text)) = snapshot.content {
            self.activity(crate::ActivityKind::Copied, &text, None, None, None);
            if let Err(e) = self.history.record(text.clone(), None, &config.settings) {
                self.fault(&e);
            }
            if allow_send {
                let targets = crate::sync::policy::targets(&config, &self.network.status())
                    .into_iter()
                    .filter(|id| {
                        self.ready
                            .get(id)
                            .is_some_and(|baseline| snapshot.revision > baseline.clipboard_revision)
                    })
                    .collect::<Vec<_>>();
                if !targets.is_empty() {
                    self.send_payload(Payload::Text(text), targets, true)
                        .await?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configuration::{
        model::{Configuration, Peer},
        runtime::{self as configuration, Change},
    };
    use crate::{Mode, PeerSettings, Settings, history::runtime as history};
    use nooboard_network::{
        ApplicationOutcome, IdentityOptions, NetworkService, Options, ReceivedContent,
        TransferStage, TrustedPeer,
    };
    use nooboard_storage::{BackendConfig, SqliteOptions, StorageService};
    use std::time::Duration;

    #[tokio::test]
    async fn enabling_automatic_targets_never_replays_pending_copies_even_with_coalesced_updates() {
        let options = || Options {
            identity: IdentityOptions::Ephemeral,
            listen: "127.0.0.1:0".parse().unwrap(),
            pairing_listen: "127.0.0.1:0".parse().unwrap(),
            discovery: false,
            ..Default::default()
        };
        let (owner_a, a, _events_a) = NetworkService::start(options()).await.unwrap();
        let (owner_b, b, mut events_b) = NetworkService::start(options()).await.unwrap();
        let trusted = |network: &Network| {
            let status = network.status();
            TrustedPeer {
                identity: status.identity,
                addresses: vec![status.listen_address],
            }
        };
        a.trust_peer(trusted(&b)).await.unwrap();
        b.trust_peer(trusted(&a)).await.unwrap();
        let mut network_state = a.subscribe();
        tokio::time::timeout(Duration::from_secs(5), async {
            while !network_state
                .borrow_and_update()
                .connections
                .iter()
                .any(|p| p.accepting)
            {
                network_state.changed().await.unwrap();
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
        let (stop, stopped) = watch::channel(false);
        let mut document = Configuration::new(Settings {
            history: false,
            ..Settings::default()
        });
        let peer = trusted(&b);
        let id = peer.identity.id.clone();
        document.peers.insert(
            id.clone(),
            Peer {
                trusted: peer,
                settings: PeerSettings {
                    address: None,
                    auto_send: true,
                },
            },
        );
        let (config, config_runtime) = configuration::channel(document, stopped.clone());
        let task = tokio::spawn(config_runtime.run(storage, stopped.clone()));
        let (history, _history_runtime) = history::channel(stopped);
        let clipboard = Arc::new(crate::tests::Clipboard::new());
        let (_channels, mut runtime) = super::super::create(
            clipboard.clone(),
            a.clone(),
            config.clone(),
            history,
            clipboard.read().await.unwrap(),
        );
        runtime.reconcile().await.unwrap();
        runtime.refresh_routes().await.unwrap();

        // Drive both ready notification orders and a watch-coalesced off/on
        // transition deterministically, without relying on select's random choice.
        for order in ["clipboard-first", "configuration-first", "coalesced"] {
            for transition in ["mode", "peer", "resume"] {
                for enabled in [false, true] {
                    let change = if transition == "peer" {
                        Change::ConfigurePeer(
                            id.clone(),
                            PeerSettings {
                                address: None,
                                auto_send: enabled,
                            },
                        )
                    } else {
                        Change::Settings(Settings {
                            mode: if transition == "mode" && !enabled {
                                Mode::Manual
                            } else {
                                Mode::Automatic
                            },
                            paused: transition == "resume" && !enabled,
                            ..config.current().settings
                        })
                    };
                    config.change(change).await.unwrap();
                    if !enabled {
                        if order != "coalesced" {
                            runtime.reconcile().await.unwrap();
                        }
                        clipboard.copy(Payload::Text("copied before enabling".into()));
                    }
                }
                if order == "configuration-first" {
                    runtime.reconcile().await.unwrap();
                }
                let before = runtime.state.borrow().operations.len();
                runtime
                    .observe(clipboard.read().await.unwrap(), true)
                    .await
                    .unwrap();
                assert_eq!(
                    runtime.state.borrow().operations.len(),
                    before,
                    "replayed pending copy: {transition}, {order}"
                );
                runtime.reconcile().await.unwrap();
                let fresh = format!("new copy: {transition}, {order}");
                clipboard.copy(Payload::Text(fresh.clone()));
                runtime
                    .observe(clipboard.read().await.unwrap(), true)
                    .await
                    .unwrap();
                let event = tokio::time::timeout(Duration::from_secs(5), events_b.next_event())
                    .await
                    .unwrap()
                    .unwrap();
                let NetworkEvent::ContentReady {
                    id,
                    transfer_id,
                    content: ReceivedContent::Text(text),
                    ..
                } = event
                else {
                    panic!("expected text content");
                };
                assert_eq!(text, fresh);
                b.complete_incoming(id, ApplicationOutcome::Applied)
                    .await
                    .unwrap();
                tokio::time::timeout(Duration::from_secs(5), async {
                    while !network_state
                        .borrow_and_update()
                        .transfers
                        .iter()
                        .any(|row| row.id == transfer_id && row.stage == TransferStage::Applied)
                    {
                        network_state.changed().await.unwrap();
                    }
                })
                .await
                .unwrap();
            }
        }
        stop.send_replace(true);
        task.await.unwrap().unwrap();
        storage_owner.shutdown().await.unwrap();
        owner_a.shutdown().await.unwrap();
        owner_b.shutdown().await.unwrap();
    }
}
