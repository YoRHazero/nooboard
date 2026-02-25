use std::collections::HashSet;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tokio::sync::{broadcast, mpsc};
use tokio::time::{MissedTickBehavior, interval};
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use tracing::{debug, warn};
use uuid::Uuid;

use crate::config::SyncConfig;
use crate::engine::{SyncEvent, TransferDirection, TransferState, TransferUpdate};
use crate::protocol::{DataPacket, Packet};

use super::SessionResult;
use super::receiver::{FileReceiverLimits, FileReceiverStateMachine, IdleTimeoutAction};
use super::sender::FileSender;
use super::stream::PriorityPacketStream;

const MAX_DATA_DRAIN_PER_LOOP: usize = 8;

#[derive(Debug)]
pub enum SessionCommand {
    SendText(String),
    SendFile(std::path::PathBuf),
    FileDecision {
        transfer_id: u32,
        accept: bool,
        reason: Option<String>,
    },
    Shutdown,
}

pub struct SessionActorContext {
    pub peer_node_id: String,
    pub config: SyncConfig,
    pub framed: Framed<tokio_rustls::TlsStream<tokio::net::TcpStream>, LengthDelimitedCodec>,
    pub command_rx: mpsc::Receiver<SessionCommand>,
    pub event_tx: mpsc::Sender<SyncEvent>,
    pub progress_tx: broadcast::Sender<TransferUpdate>,
    pub shutdown_rx: broadcast::Receiver<()>,
}

pub async fn run_session_actor(mut ctx: SessionActorContext) -> SessionResult<()> {
    let mut stream = PriorityPacketStream::new(ctx.framed);
    let mut sender = FileSender::new();
    let mut seen_text_ids = HashSet::new();

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
                &ctx.peer_node_id,
                stream.remaining_data_capacity() > 0,
            )
            .await?;

        drain_sender_data_packets(&mut sender, &mut stream);

        while let Some(update) = sender.pop_update() {
            emit_transfer_update(
                &ctx.peer_node_id,
                TransferDirection::Outgoing,
                update.transfer_id,
                update.state,
                &ctx.event_tx,
                &ctx.progress_tx,
            )
            .await;
        }

        let _ = stream.flush_one().await?;

        tokio::select! {
            _ = ctx.shutdown_rx.recv() => {
                debug!(peer=%ctx.peer_node_id, "session actor received shutdown");
                break;
            }
            _ = ping_timer.tick() => {
                if !stream.try_queue_control(Packet::Ping { timestamp: now_millis_u64() }) {
                    warn!(peer=%ctx.peer_node_id, "drop ping because control queue is full");
                }
            }
            _ = idle_timer.tick() => {
                let actions = receiver
                    .collect_idle_actions(Duration::from_millis(ctx.config.transfer_idle_timeout_ms))
                    .await?;
                for action in actions {
                    match action {
                        IdleTimeoutAction::RejectDecision { transfer_id, reason } => {
                            if !stream.try_queue_data(Packet::Data(DataPacket::FileDecision {
                                transfer_id,
                                accept: false,
                                reason: Some(reason.clone()),
                            })) {
                                warn!(peer=%ctx.peer_node_id, transfer_id, "drop timeout FileDecision because data queue is full");
                            }
                            emit_transfer_update(
                                &ctx.peer_node_id,
                                TransferDirection::Incoming,
                                transfer_id,
                                TransferState::Cancelled {
                                    reason: Some(reason),
                                },
                                &ctx.event_tx,
                                &ctx.progress_tx,
                            )
                            .await;
                        }
                        IdleTimeoutAction::CancelTransfer { transfer_id, reason } => {
                            warn!(peer=%ctx.peer_node_id, transfer_id, "{reason}");
                            if !stream.try_queue_data(Packet::Data(DataPacket::FileCancel { transfer_id })) {
                                warn!(peer=%ctx.peer_node_id, transfer_id, "drop timeout FileCancel because data queue is full");
                            }
                            emit_transfer_update(
                                &ctx.peer_node_id,
                                TransferDirection::Incoming,
                                transfer_id,
                                TransferState::Failed { reason },
                                &ctx.event_tx,
                                &ctx.progress_tx,
                            )
                            .await;
                        }
                    }
                }
            }
            maybe_command = ctx.command_rx.recv() => {
                match maybe_command {
                    Some(SessionCommand::SendText(content)) => {
                        if !stream.try_queue_data(Packet::Data(DataPacket::ClipboardText {
                            id: Uuid::now_v7().to_string(),
                            content,
                        })) {
                            warn!(peer=%ctx.peer_node_id, "drop outgoing text because data queue is full");
                        }
                    }
                    Some(SessionCommand::SendFile(path)) => {
                        sender.enqueue_file(path);
                    }
                    Some(SessionCommand::FileDecision { transfer_id, accept, reason }) => {
                        match receiver.apply_decision(transfer_id, accept).await {
                            Ok(()) => {
                                if !stream.try_queue_data(Packet::Data(DataPacket::FileDecision {
                                    transfer_id,
                                    accept,
                                    reason: reason.clone(),
                                })) {
                                    warn!(peer=%ctx.peer_node_id, transfer_id, "drop local FileDecision because data queue is full");
                                }
                                if !accept {
                                    emit_transfer_update(
                                        &ctx.peer_node_id,
                                        TransferDirection::Incoming,
                                        transfer_id,
                                        TransferState::Cancelled { reason },
                                        &ctx.event_tx,
                                        &ctx.progress_tx,
                                    )
                                    .await;
                                }
                            }
                            Err(error) => {
                                warn!(peer=%ctx.peer_node_id, transfer_id, "invalid local file decision: {error}");
                            }
                        }
                    }
                    Some(SessionCommand::Shutdown) | None => break,
                }
            }
            maybe_packet = stream.recv() => {
                match maybe_packet? {
                    Some(packet) => {
                        if handle_incoming_packet(
                            &ctx.peer_node_id,
                            packet,
                            &mut stream,
                            &mut sender,
                            &mut receiver,
                            &mut seen_text_ids,
                            &ctx.event_tx,
                            &ctx.progress_tx,
                        ).await? {
                            last_pong = Instant::now();
                        }
                    }
                    None => break,
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(1)), if stream.has_pending() => {}
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
    peer_node_id: &str,
    packet: Packet,
    stream: &mut PriorityPacketStream,
    sender: &mut FileSender,
    receiver: &mut FileReceiverStateMachine,
    seen_text_ids: &mut HashSet<String>,
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
            if !stream.try_queue_control(Packet::Pong { timestamp }) {
                warn!(peer=%peer_node_id, "drop pong because control queue is full");
            }
            return Ok(false);
        }
        Packet::Pong { .. } => {
            return Ok(true);
        }
        Packet::Data(data_packet) => match data_packet {
            DataPacket::ClipboardText { id, content } => {
                if seen_text_ids.insert(id) {
                    let _ = event_tx.send(SyncEvent::TextReceived(content)).await;
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
                                peer_node_id: peer_node_id.to_string(),
                                transfer_id: request.transfer_id,
                                file_name: request.file_name,
                                file_size: request.file_size,
                                total_chunks: request.total_chunks,
                            })
                            .await;
                        emit_transfer_update(
                            peer_node_id,
                            TransferDirection::Incoming,
                            transfer_id,
                            TransferState::Started {
                                file_name,
                                total_bytes: file_size,
                            },
                            event_tx,
                            progress_tx,
                        )
                        .await;
                    }
                    Err(error) => {
                        if !stream.try_queue_data(Packet::Data(DataPacket::FileDecision {
                            transfer_id,
                            accept: false,
                            reason: Some(error.to_string()),
                        })) {
                            warn!(peer=%peer_node_id, transfer_id, "drop auto reject FileDecision because data queue is full");
                        }
                        emit_transfer_update(
                            peer_node_id,
                            TransferDirection::Incoming,
                            transfer_id,
                            TransferState::Failed {
                                reason: error.to_string(),
                            },
                            event_tx,
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
                        peer_node_id,
                        TransferDirection::Incoming,
                        transfer_id,
                        TransferState::Progress {
                            done_bytes: progress.done_bytes,
                            total_bytes: progress.total_bytes,
                            bps: None,
                            eta_ms: None,
                        },
                        event_tx,
                        progress_tx,
                    )
                    .await;
                }
                Err(error) => {
                    warn!(transfer_id, "file chunk failed: {error}");
                    if !stream.try_queue_data(Packet::Data(DataPacket::FileCancel { transfer_id }))
                    {
                        warn!(peer=%peer_node_id, transfer_id, "drop FileCancel after chunk failure because data queue is full");
                    }
                    let _ = receiver.abort_transfer(transfer_id).await;
                    emit_transfer_update(
                        peer_node_id,
                        TransferDirection::Incoming,
                        transfer_id,
                        TransferState::Failed {
                            reason: error.to_string(),
                        },
                        event_tx,
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
                        peer_node_id,
                        TransferDirection::Incoming,
                        transfer_id,
                        TransferState::Finished {
                            path: Some(downloaded.path),
                        },
                        event_tx,
                        progress_tx,
                    )
                    .await;
                }
                Err(error) => {
                    warn!(transfer_id, "file end failed: {error}");
                    if !stream.try_queue_data(Packet::Data(DataPacket::FileCancel { transfer_id }))
                    {
                        warn!(peer=%peer_node_id, transfer_id, "drop FileCancel after file end failure because data queue is full");
                    }
                    let _ = receiver.abort_transfer(transfer_id).await;
                    emit_transfer_update(
                        peer_node_id,
                        TransferDirection::Incoming,
                        transfer_id,
                        TransferState::Failed {
                            reason: error.to_string(),
                        },
                        event_tx,
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
                        peer_node_id,
                        TransferDirection::Incoming,
                        transfer_id,
                        TransferState::Cancelled {
                            reason: Some("cancelled by peer".to_string()),
                        },
                        event_tx,
                        progress_tx,
                    )
                    .await;
                }
            }
        },
    }

    Ok(false)
}

fn drain_sender_data_packets(sender: &mut FileSender, stream: &mut PriorityPacketStream) {
    let mut drained = 0;
    while drained < MAX_DATA_DRAIN_PER_LOOP {
        let Some(packet) = sender.pop_packet() else {
            break;
        };

        if !stream.try_queue_data(packet.clone()) {
            sender.requeue_packet_front(packet);
            break;
        }
        drained += 1;
    }
}

async fn emit_transfer_update(
    peer_node_id: &str,
    direction: TransferDirection,
    transfer_id: u32,
    state: TransferState,
    event_tx: &mpsc::Sender<SyncEvent>,
    progress_tx: &broadcast::Sender<TransferUpdate>,
) {
    let update = TransferUpdate {
        transfer_id,
        peer_node_id: peer_node_id.to_string(),
        direction,
        state,
    };

    if matches!(update.state, TransferState::Progress { .. }) {
        let _ = progress_tx.send(update);
    } else {
        let _ = event_tx.send(SyncEvent::TransferUpdate(update)).await;
    }
}
