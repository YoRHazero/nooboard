use std::collections::{HashSet, VecDeque};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures::StreamExt;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::time::{MissedTickBehavior, interval};
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use tracing::{debug, warn};

use crate::config::SyncConfig;
use crate::engine::{SendTextRequest, SyncEvent, TransferDirection, TransferState, TransferUpdate};
use crate::error::{ConnectionError, TransportError};
use crate::protocol::{DataPacket, Packet, decode_packet};

use super::SessionResult;
use super::outbox::PacketOutbox;
use super::receiver::{FileReceiverLimits, FileReceiverStateMachine, IdleTimeoutAction};
use super::sender::FileSender;

const MAX_DATA_DRAIN_PER_LOOP: usize = 8;
const SEEN_TEXT_IDS_LIMIT: usize = 4096;

#[derive(Debug)]
struct SeenTextIdCache {
    seen: HashSet<String>,
    order: VecDeque<String>,
    limit: usize,
}

impl SeenTextIdCache {
    fn new(limit: usize) -> Self {
        Self {
            seen: HashSet::new(),
            order: VecDeque::new(),
            limit,
        }
    }

    fn insert(&mut self, event_id: String) -> bool {
        if self.seen.contains(&event_id) {
            return false;
        }

        self.order.push_back(event_id.clone());
        self.seen.insert(event_id);

        while self.order.len() > self.limit {
            if let Some(oldest) = self.order.pop_front() {
                self.seen.remove(&oldest);
            }
        }

        true
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.seen.len()
    }
}

#[derive(Debug)]
pub enum SessionCommand {
    SendText(SendTextRequest),
    SendFile {
        transfer_id: u32,
        path: std::path::PathBuf,
    },
    FileDecision {
        transfer_id: u32,
        accept: bool,
        reason: Option<String>,
    },
    CancelTransfer {
        transfer_id: u32,
        reply: oneshot::Sender<Result<(), ConnectionError>>,
    },
    Shutdown,
}

pub struct SessionActorContext {
    pub peer_noob_id: String,
    pub peer_device_id: String,
    pub config: SyncConfig,
    pub framed: Framed<tokio_rustls::TlsStream<tokio::net::TcpStream>, LengthDelimitedCodec>,
    pub command_rx: mpsc::Receiver<SessionCommand>,
    pub event_tx: mpsc::Sender<SyncEvent>,
    pub progress_tx: broadcast::Sender<TransferUpdate>,
    pub shutdown_rx: broadcast::Receiver<()>,
}

pub async fn run_session_actor(mut ctx: SessionActorContext) -> SessionResult<()> {
    let (mut sink, mut stream_reader) = ctx.framed.split();
    let (writer_tx, mut writer_rx) = mpsc::channel::<Packet>(1);
    let (writer_error_tx, mut writer_error_rx) = mpsc::channel::<ConnectionError>(1);

    tokio::spawn(async move {
        while let Some(packet) = writer_rx.recv().await {
            if let Err(error) = crate::transport::send_packet_sink(&mut sink, &packet).await {
                let _ = writer_error_tx.send(ConnectionError::from(error)).await;
                break;
            }
        }
    });

    let mut outbox = PacketOutbox::new();

    let mut sender = FileSender::new();
    let mut seen_text_ids = SeenTextIdCache::new(SEEN_TEXT_IDS_LIMIT);

    let mut receiver = FileReceiverStateMachine::new(FileReceiverLimits {
        download_dir: ctx.config.download_dir.clone(),
        max_file_size: ctx.config.max_file_size,
        active_downloads: ctx.config.active_downloads,
    });

    let mut ping_timer = interval(Duration::from_millis(ctx.config.ping_interval_ms));
    ping_timer.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let mut idle_timer = interval(Duration::from_millis(500));
    idle_timer.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let mut last_pong = Instant::now();

    loop {
        if Instant::now().duration_since(last_pong)
            > Duration::from_millis(ctx.config.pong_timeout_ms)
        {
            return Err(crate::error::ConnectionError::PongTimeout);
        }

        sender
            .tick(
                &ctx.config,
                &ctx.peer_noob_id,
                outbox.remaining_data_capacity() > 0,
            )
            .await?;

        // Sender -> Outbox
        let mut drained = 0;
        while drained < MAX_DATA_DRAIN_PER_LOOP {
            let Some(packet) = sender.pop_packet() else {
                break;
            };

            match outbox.queue_data(packet) {
                Ok(()) => drained += 1,
                Err(packet) => {
                    sender.requeue_packet_front(packet);
                    break;
                }
            }
        }

        while let Some(update) = sender.pop_update() {
            emit_transfer_update(
                &ctx.peer_noob_id,
                TransferDirection::Outgoing,
                update.transfer_id,
                update.state,
                &ctx.progress_tx,
            )
            .await;
        }

        tokio::select! {
            // 1. outbox -> writer
            res = writer_tx.reserve(), if outbox.has_pending() => {
                match res {
                    Ok(permit) => {
                        if let Some(packet) = outbox.pop_next() {
                            permit.send(packet);
                        }
                    }
                    Err(error) => {
                        return Err(ConnectionError::State(format!(
                            "outbox writer channel closed: {error}"
                        )));
                    }
                }
            }

            // 1.1 writer transport errors
            maybe_writer_error = writer_error_rx.recv() => {
                match maybe_writer_error {
                    Some(error) => return Err(error),
                    None => {
                        return Err(ConnectionError::State(
                            "writer task exited unexpectedly".to_string(),
                        ))
                    }
                }
            }

            // 2. receive data from peer
            maybe_bytes = stream_reader.next() => {
                match maybe_bytes {
                    Some(Ok(bytes)) => {
                        let packet = decode_packet(&bytes)
                            .map_err(TransportError::Protocol)
                            .map_err(ConnectionError::from)?;

                        if handle_incoming_packet(
                            &ctx.peer_noob_id,
                            &ctx.peer_device_id,
                            packet,
                            &mut outbox,
                            &mut sender,
                            &mut receiver,
                            &mut seen_text_ids,
                            &ctx.event_tx,
                            &ctx.progress_tx,
                        ).await? {
                            last_pong = Instant::now();
                        }
                    }
                    Some(Err(error)) => return Err(ConnectionError::Io(error)),
                    None => break,
                }
            }

            // 3. Command from peer
            _ = ctx.shutdown_rx.recv() => {
                debug!(peer=%ctx.peer_noob_id, "shutdown signal received");
                break;
            }

            // 4. periodic ping
            _ = ping_timer.tick() => {
                if !outbox.queue_control(Packet::Ping { timestamp: now_millis_u64() }) {
                    warn!(peer=%ctx.peer_noob_id, "drop ping because control queue is full");
                }
            }

            // 5. periodic idle check
            _ = idle_timer.tick() => {
                let actions = receiver
                    .collect_idle_actions(Duration::from_millis(ctx.config.transfer_idle_timeout_ms))
                    .await?;
                for action in actions {
                    match action {
                        IdleTimeoutAction::RejectDecision { transfer_id, reason } => {
                            if outbox.queue_data(Packet::Data(DataPacket::FileDecision {
                                transfer_id,
                                accept: false,
                                reason: Some(reason.clone()),
                            })).is_err() {
                                warn!(peer=%ctx.peer_noob_id, transfer_id, "drop timeout FileDecision because data queue is full");
                            }
                            emit_transfer_update(
                                &ctx.peer_noob_id,
                                TransferDirection::Incoming,
                                transfer_id,
                                TransferState::Rejected {
                                    reason: Some(reason),
                                },
                                &ctx.progress_tx,
                            )
                            .await;
                        }
                        IdleTimeoutAction::CancelTransfer { transfer_id, reason } => {
                            warn!(peer=%ctx.peer_noob_id, transfer_id, "{reason}");
                            if outbox.queue_data(Packet::Data(DataPacket::FileCancel { transfer_id })).is_err() {
                                warn!(peer=%ctx.peer_noob_id, transfer_id, "drop timeout FileCancel because data queue is full");
                            }
                            emit_transfer_update(
                                &ctx.peer_noob_id,
                                TransferDirection::Incoming,
                                transfer_id,
                                TransferState::Failed { reason },
                                &ctx.progress_tx,
                            )
                            .await;
                        }
                    }
                }
            }

            // 6. command from caller
            maybe_command = ctx.command_rx.recv() => {
                match maybe_command {
                    Some(SessionCommand::SendText(request)) => {
                        if outbox.queue_data(Packet::Data(DataPacket::ClipboardText {
                            event_id: request.event_id,
                            content: request.content,
                        })).is_err() {
                            warn!(peer=%ctx.peer_noob_id, "drop outgoing text because data queue is full");
                        }
                    }
                    Some(SessionCommand::SendFile { transfer_id, path }) => {
                        sender.enqueue_file(transfer_id, path);
                    }
                    Some(SessionCommand::FileDecision { transfer_id, accept, reason }) => {
                        match receiver.apply_decision(transfer_id, accept).await {
                            Ok(()) => {
                                if outbox.queue_data(Packet::Data(DataPacket::FileDecision {
                                    transfer_id,
                                    accept,
                                    reason: reason.clone(),
                                })).is_err() {
                                    warn!(peer=%ctx.peer_noob_id, transfer_id, "drop local FileDecision because data queue is full");
                                }
                            }
                            Err(error) => {
                                warn!(peer=%ctx.peer_noob_id, transfer_id, "invalid local file decision: {error}");
                            }
                        }
                    }
                    Some(SessionCommand::CancelTransfer { transfer_id, reply }) => {
                        let cancel_reason = Some("cancelled by local peer".to_string());
                        if sender.cancel_transfer(transfer_id, cancel_reason.clone()) {
                            let _ = reply.send(Ok(()));
                            continue;
                        }

                        match receiver.handle_file_cancel_with_flag(transfer_id).await {
                            Ok(true) => {
                                if outbox.queue_data(Packet::Data(DataPacket::FileCancel { transfer_id })).is_err() {
                                    warn!(peer=%ctx.peer_noob_id, transfer_id, "drop local FileCancel because data queue is full");
                                }
                                emit_transfer_update(
                                    &ctx.peer_noob_id,
                                    TransferDirection::Incoming,
                                    transfer_id,
                                    TransferState::Cancelled {
                                        reason: cancel_reason,
                                    },
                                    &ctx.progress_tx,
                                )
                                .await;
                                let _ = reply.send(Ok(()));
                            }
                            Ok(false) => {
                                let _ = reply.send(Err(ConnectionError::State(format!(
                                    "transfer {transfer_id} cannot be cancelled"
                                ))));
                            }
                            Err(error) => {
                                let _ = reply.send(Err(ConnectionError::from(error)));
                            }
                        }
                    }
                    Some(SessionCommand::Shutdown) | None => break,
                }
            }
        }
    }

    receiver.cleanup_all().await;
    Ok(())
}

fn now_millis_u64() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);

    if millis > u64::MAX as u128 {
        u64::MAX
    } else {
        millis as u64
    }
}

async fn handle_incoming_packet(
    peer_noob_id: &str,
    peer_device_id: &str,
    packet: Packet,
    outbox: &mut PacketOutbox,
    sender: &mut FileSender,
    receiver: &mut FileReceiverStateMachine,
    seen_text_ids: &mut SeenTextIdCache,
    event_tx: &mpsc::Sender<SyncEvent>,
    progress_tx: &broadcast::Sender<TransferUpdate>,
) -> SessionResult<bool> {
    match packet {
        Packet::Handshake(_) => {
            return Err(crate::error::ConnectionError::State(
                "received handshake packet after authentication".to_string(),
            ));
        }
        Packet::Ping { timestamp } => {
            if !outbox.queue_control(Packet::Pong { timestamp }) {
                warn!(peer=%peer_noob_id, "drop pong because control queue is full");
            }
            return Ok(false);
        }
        Packet::Pong { .. } => {
            return Ok(true);
        }
        Packet::Data(data_packet) => match data_packet {
            DataPacket::ClipboardText { event_id, content } => {
                if seen_text_ids.insert(event_id.clone()) {
                    let _ = event_tx
                        .send(SyncEvent::TextReceived {
                            event_id,
                            content,
                            noob_id: peer_noob_id.to_string(),
                            device_id: peer_device_id.to_string(),
                        })
                        .await;
                }
            }
            DataPacket::FileStart {
                transfer_id,
                file_name,
                file_size,
                total_chunks,
            } => {
                match receiver
                    .register_file_start(transfer_id, &file_name, file_size, total_chunks)
                    .await
                {
                    Ok(request) => {
                        let _ = event_tx
                            .send(SyncEvent::FileDecisionRequired {
                                peer_noob_id: peer_noob_id.to_string(),
                                transfer_id: request.transfer_id,
                                file_name: request.file_name,
                                file_size: request.file_size,
                                total_chunks: request.total_chunks,
                            })
                            .await;
                        emit_transfer_update(
                            peer_noob_id,
                            TransferDirection::Incoming,
                            transfer_id,
                            TransferState::Started {
                                file_name,
                                total_bytes: file_size,
                            },
                            progress_tx,
                        )
                        .await;
                    }
                    Err(error) => {
                        let packet = Packet::Data(DataPacket::FileDecision {
                            transfer_id,
                            accept: false,
                            reason: Some(error.to_string()),
                        });

                        if outbox.queue_data(packet).is_err() {
                            warn!(peer=%peer_noob_id, transfer_id, "drop auto-reject FileDecision");
                        }
                        emit_transfer_update(
                            peer_noob_id,
                            TransferDirection::Incoming,
                            transfer_id,
                            TransferState::Failed {
                                reason: error.to_string(),
                            },
                            progress_tx,
                        )
                        .await;
                    }
                }
            }
            DataPacket::FileDecision {
                transfer_id,
                accept,
                reason,
            } => {
                sender.on_file_decision(transfer_id, accept, reason);
            }
            DataPacket::FileChunk {
                transfer_id,
                seq,
                data,
            } => match receiver.handle_file_chunk(transfer_id, seq, &data).await {
                Ok(progress) => {
                    emit_transfer_update(
                        peer_noob_id,
                        TransferDirection::Incoming,
                        transfer_id,
                        TransferState::Progress {
                            done_bytes: progress.done_bytes,
                            total_bytes: progress.total_bytes,
                            bps: None,
                            eta_ms: None,
                        },
                        progress_tx,
                    )
                    .await;
                }
                Err(error) => {
                    warn!(transfer_id, "file chunk failed: {error}");
                    if outbox
                        .queue_data(Packet::Data(DataPacket::FileCancel { transfer_id }))
                        .is_err()
                    {
                        warn!(peer=%peer_noob_id, transfer_id, "drop FileCancel after chunk failure because data queue is full");
                    }
                    let _ = receiver.abort_transfer(transfer_id).await;
                    emit_transfer_update(
                        peer_noob_id,
                        TransferDirection::Incoming,
                        transfer_id,
                        TransferState::Failed {
                            reason: error.to_string(),
                        },
                        progress_tx,
                    )
                    .await;
                }
            },
            DataPacket::FileEnd {
                transfer_id,
                checksum,
            } => match receiver.handle_file_end(transfer_id, &checksum).await {
                Ok(downloaded) => {
                    emit_transfer_update(
                        peer_noob_id,
                        TransferDirection::Incoming,
                        transfer_id,
                        TransferState::Finished {
                            path: Some(downloaded.path),
                        },
                        progress_tx,
                    )
                    .await;
                }
                Err(error) => {
                    warn!(transfer_id, "file end failed: {error}");
                    if outbox
                        .queue_data(Packet::Data(DataPacket::FileCancel { transfer_id }))
                        .is_err()
                    {
                        warn!(peer=%peer_noob_id, transfer_id, "drop FileCancel after file end failure because data queue is full");
                    }
                    let _ = receiver.abort_transfer(transfer_id).await;
                    emit_transfer_update(
                        peer_noob_id,
                        TransferDirection::Incoming,
                        transfer_id,
                        TransferState::Failed {
                            reason: error.to_string(),
                        },
                        progress_tx,
                    )
                    .await;
                }
            },
            DataPacket::FileCancel { transfer_id } => {
                if receiver
                    .handle_file_cancel_with_flag(transfer_id)
                    .await
                    .unwrap_or(false)
                {
                    emit_transfer_update(
                        peer_noob_id,
                        TransferDirection::Incoming,
                        transfer_id,
                        TransferState::Cancelled {
                            reason: Some("cancelled by peer".to_string()),
                        },
                        progress_tx,
                    )
                    .await;
                }
            }
        },
    }

    Ok(true)
}

async fn emit_transfer_update(
    peer_noob_id: &str,
    direction: TransferDirection,
    transfer_id: u32,
    state: TransferState,
    progress_tx: &broadcast::Sender<TransferUpdate>,
) {
    let update = TransferUpdate {
        transfer_id,
        peer_noob_id: peer_noob_id.to_string(),
        direction,
        state,
    };

    let _ = progress_tx.send(update);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seen_text_id_cache_is_bounded() {
        let mut cache = SeenTextIdCache::new(2);

        assert!(cache.insert("event-a".to_string()));
        assert!(cache.insert("event-b".to_string()));
        assert_eq!(cache.len(), 2);

        assert!(!cache.insert("event-a".to_string()));
        assert_eq!(cache.len(), 2);

        assert!(cache.insert("event-c".to_string()));
        assert_eq!(cache.len(), 2);
        assert!(cache.insert("event-a".to_string()));
        assert_eq!(cache.len(), 2);
    }
}
