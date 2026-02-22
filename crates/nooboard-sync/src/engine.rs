use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use nooboard_core::ClipboardEvent;
use nooboard_platform::{ClipboardBackend, DEFAULT_WATCH_INTERVAL};
use nooboard_storage::ClipboardRepository;
use tokio::sync::mpsc;
use tracing::info;

use crate::discovery::{DiscoveryConfig, start_mdns};
use crate::error::SyncError;
use crate::protocol::SyncEvent;
use crate::transport::{TransportConfig, start_transport};

const REMOTE_SET_SUPPRESSION_WINDOW: Duration = Duration::from_secs(2);

#[derive(Debug, Clone)]
pub struct SyncConfig {
    pub device_id: String,
    pub listen_addr: SocketAddr,
    pub token: String,
    pub peers: Vec<SocketAddr>,
    pub mdns_enabled: bool,
}

pub struct SyncEngine<'a> {
    backend: &'a dyn ClipboardBackend,
    repository: &'a dyn ClipboardRepository,
}

impl<'a> SyncEngine<'a> {
    pub fn new(backend: &'a dyn ClipboardBackend, repository: &'a dyn ClipboardRepository) -> Self {
        Self {
            backend,
            repository,
        }
    }

    pub async fn run(self, config: SyncConfig) -> Result<(), SyncError> {
        let mut transport = start_transport(TransportConfig {
            device_id: config.device_id.clone(),
            token: config.token.clone(),
            listen_addr: config.listen_addr,
            peers: config.peers.clone(),
        })
        .await?;

        for peer in &config.peers {
            let _ = transport.peer_tx.send(*peer);
        }

        let _mdns_handle = if config.mdns_enabled {
            let discovery = DiscoveryConfig::new(&config.device_id, config.listen_addr);
            Some(start_mdns(&discovery, transport.peer_tx.clone())?)
        } else {
            None
        };

        info!(
            "sync engine started, device_id={}, listen={}, mdns={}",
            config.device_id, config.listen_addr, config.mdns_enabled
        );

        let (watch_sender, mut watch_receiver) = mpsc::channel::<ClipboardEvent>(64);
        let shutdown = Arc::new(AtomicBool::new(false));
        let observer = self.backend.watch_changes(
            watch_sender,
            Arc::clone(&shutdown),
            DEFAULT_WATCH_INTERVAL,
        )?;
        let mut next_seq = 1_u64;
        let mut suppressed_content: VecDeque<(String, Instant)> = VecDeque::new();

        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    shutdown.store(true, Ordering::Relaxed);
                    break;
                }
                maybe_local = watch_receiver.recv() => {
                    if let Some(event) = maybe_local {
                        if should_suppress_local(&mut suppressed_content, &event.text) {
                            continue;
                        }

                        let captured_at = i64::try_from(event.timestamp_millis())
                            .map_err(|_| SyncError::Protocol("local event timestamp overflowed i64".to_string()))?;
                        self.repository.insert_text_event(&event.text, captured_at)?;
                        let outbound = SyncEvent::new(
                            &config.device_id,
                            next_seq,
                            captured_at,
                            event.text,
                        );
                        next_seq = next_seq.saturating_add(1);
                        let _ = transport.broadcast_event(outbound);
                    }
                }
                maybe_remote = transport.recv_event() => {
                    if let Some(remote_event) = maybe_remote {
                        self.apply_remote_event(&remote_event, &mut suppressed_content)?;
                    }
                }
            }
        }

        shutdown.store(true, Ordering::Relaxed);
        let _ = observer.join();
        Ok(())
    }

    fn apply_remote_event(
        &self,
        event: &SyncEvent,
        suppressed_content: &mut VecDeque<(String, Instant)>,
    ) -> Result<(), SyncError> {
        let seen_at = now_millis();
        let is_first_seen =
            self.repository
                .mark_seen_event(&event.origin_device_id, event.origin_seq, seen_at)?;
        if !is_first_seen {
            return Ok(());
        }

        let current_latest = self.repository.latest_content()?;
        let should_set = current_latest.as_deref() != Some(event.content.as_str());
        if should_set {
            self.backend.write_text(&event.content)?;
            push_suppression(suppressed_content, &event.content);
        }
        self.repository
            .insert_text_event(&event.content, event.captured_at)?;
        Ok(())
    }
}

fn now_millis() -> i64 {
    let now = std::time::SystemTime::now();
    now.duration_since(std::time::UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

fn push_suppression(queue: &mut VecDeque<(String, Instant)>, content: &str) {
    prune_suppression(queue);
    queue.push_back((content.to_string(), Instant::now()));
}

fn should_suppress_local(queue: &mut VecDeque<(String, Instant)>, local_content: &str) -> bool {
    prune_suppression(queue);
    queue.iter().any(|(content, _)| content == local_content)
}

fn prune_suppression(queue: &mut VecDeque<(String, Instant)>) {
    let now = Instant::now();
    while let Some((_, created_at)) = queue.front() {
        if now.duration_since(*created_at) > REMOTE_SET_SUPPRESSION_WINDOW {
            queue.pop_front();
        } else {
            break;
        }
    }
}
