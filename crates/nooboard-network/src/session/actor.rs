use std::time::{Duration, Instant};

use futures::StreamExt;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::time::{MissedTickBehavior, interval};

use crate::config::NetworkConfig;
use crate::errors::ConnectionError;
use crate::protocol::{ControlPacket, DataPacket, Packet};
use crate::transport::{NetworkFramed, send_packet_sink};
use crate::{
    ActiveTransferInfo, ActiveTransferState, CompletedTransferInfo, IncomingTransferOffer,
    NetworkEvent, SessionId, TransferDirection, TransferOutcome, TransferTicket,
};

use super::outbox::PacketOutbox;
use super::receiver::{FileReceiverLimits, FileReceiverStateMachine, IdleTimeoutAction};
use super::sender::{FileSender, TransferProgressEvent};

const MAX_DATA_DRAIN_PER_LOOP: usize = 8;

#[derive(Debug)]
pub(crate) enum SessionCommand {
    SendText {
        event_id: String,
        content: String,
    },
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

#[derive(Debug)]
pub(crate) enum SessionLifecycleEvent {
    Network(NetworkEvent),
    Failed {
        session_id: SessionId,
        error: ConnectionError,
    },
    Disconnected {
        session_id: SessionId,
    },
}

pub(crate) struct SessionActorContext {
    pub(crate) session_id: SessionId,
    pub(crate) peer_noob_id: String,
    pub(crate) peer_device_id: String,
    pub(crate) config: NetworkConfig,
    pub(crate) framed: NetworkFramed,
    pub(crate) command_rx: mpsc::Receiver<SessionCommand>,
    pub(crate) lifecycle_tx: mpsc::Sender<SessionLifecycleEvent>,
    pub(crate) shutdown_rx: broadcast::Receiver<()>,
}

pub(crate) async fn run_session_actor(ctx: SessionActorContext) {
    let session_id = ctx.session_id;
    let lifecycle_tx = ctx.lifecycle_tx.clone();
    let result = run_session_actor_inner(ctx).await;
    let lifecycle = match result {
        Ok(()) => SessionLifecycleEvent::Disconnected { session_id },
        Err(error) => SessionLifecycleEvent::Failed { session_id, error },
    };
    let _ = lifecycle_tx.send(lifecycle).await;
}

async fn run_session_actor_inner(ctx: SessionActorContext) -> Result<(), ConnectionError> {
    let SessionActorContext {
        session_id,
        peer_noob_id,
        peer_device_id,
        config,
        framed,
        mut command_rx,
        lifecycle_tx,
        mut shutdown_rx,
    } = ctx;

    let (mut sink, mut stream_reader) = framed.split();
    let (writer_tx, mut writer_rx) = mpsc::channel::<Packet>(1);
    let (writer_error_tx, mut writer_error_rx) = mpsc::channel::<ConnectionError>(1);

    tokio::spawn(async move {
        while let Some(packet) = writer_rx.recv().await {
            if let Err(error) = send_packet_sink(&mut sink, &packet).await {
                let _ = writer_error_tx.send(ConnectionError::from(error)).await;
                break;
            }
        }
    });

    let mut outbox = PacketOutbox::new();
    let mut sender = FileSender::new();
    let mut receiver = FileReceiverStateMachine::new(FileReceiverLimits {
        download_dir: config.transfer.download_dir.clone(),
        max_file_size: config.transfer.max_file_size,
        active_downloads: config.transfer.active_downloads,
    });

    let mut ping_timer = interval(Duration::from_millis(config.transport.ping_interval_ms));
    ping_timer.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let mut idle_timer = interval(Duration::from_millis(500));
    idle_timer.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let mut last_pong = Instant::now();

    loop {
        if Instant::now().duration_since(last_pong)
            > Duration::from_millis(config.transport.pong_timeout_ms)
        {
            return Err(ConnectionError::PongTimeout);
        }

        sender
            .tick(&config, outbox.remaining_data_capacity() > 0)
            .await?;

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
            emit_sender_update(
                &lifecycle_tx,
                session_id,
                &peer_noob_id,
                &peer_device_id,
                update,
            )
            .await;
        }

        tokio::select! {
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
            maybe_writer_error = writer_error_rx.recv() => {
                match maybe_writer_error {
                    Some(error) => return Err(error),
                    None => return Err(ConnectionError::State(
                        "writer task exited unexpectedly".to_string(),
                    )),
                }
            }
            maybe_bytes = stream_reader.next() => {
                match maybe_bytes {
                    Some(Ok(bytes)) => {
                        let packet = crate::protocol::decode_packet(&bytes)
                            .map_err(crate::errors::TransportError::Protocol)
                            .map_err(ConnectionError::from)?;

                        if handle_incoming_packet(
                            &lifecycle_tx,
                            session_id,
                            &peer_noob_id,
                            &peer_device_id,
                            packet,
                            &mut outbox,
                            &mut sender,
                            &mut receiver,
                        ).await? {
                            last_pong = Instant::now();
                        }
                    }
                    Some(Err(error)) => return Err(ConnectionError::Io(error)),
                    None => break,
                }
            }
            _ = shutdown_rx.recv() => {
                break;
            }
            _ = ping_timer.tick() => {
                let _ = outbox.queue_control(Packet::Ping {
                    timestamp: now_millis_u64(),
                });
            }
            _ = idle_timer.tick() => {
                let actions = receiver
                    .collect_idle_actions(Duration::from_millis(config.transfer.idle_timeout_ms))
                    .await?;
                for action in actions {
                    match action {
                        IdleTimeoutAction::RejectDecision { transfer_id, reason } => {
                            let _ = outbox.queue_data(Packet::Data(DataPacket::FileDecision {
                                transfer_id,
                                accept: false,
                                reason: Some(reason.clone()),
                            }));
                            emit_transfer_completed(
                                &lifecycle_tx,
                                session_id,
                                &peer_noob_id,
                                &peer_device_id,
                                transfer_id,
                                TransferDirection::Download,
                                TransferOutcome::Rejected,
                                None,
                                Some(reason),
                            )
                            .await;
                        }
                        IdleTimeoutAction::CancelTransfer { transfer_id, reason } => {
                            let _ = outbox.queue_data(Packet::Data(DataPacket::FileCancel { transfer_id }));
                            emit_transfer_completed(
                                &lifecycle_tx,
                                session_id,
                                &peer_noob_id,
                                &peer_device_id,
                                transfer_id,
                                TransferDirection::Download,
                                TransferOutcome::Failed,
                                None,
                                Some(reason),
                            )
                            .await;
                        }
                    }
                }
            }
            maybe_command = command_rx.recv() => {
                match maybe_command {
                    Some(SessionCommand::SendText { event_id, content }) => {
                        let _ = outbox.queue_data(Packet::Data(DataPacket::ClipboardText {
                            event_id,
                            content,
                        }));
                    }
                    Some(SessionCommand::SendFile { transfer_id, path }) => {
                        sender.enqueue_file(transfer_id, path);
                    }
                    Some(SessionCommand::FileDecision { transfer_id, accept, reason }) => {
                        if receiver.apply_decision(transfer_id, accept).await.is_ok() {
                            let _ = outbox.queue_data(Packet::Data(DataPacket::FileDecision {
                                transfer_id,
                                accept,
                                reason,
                            }));
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
                                let _ = outbox.queue_data(Packet::Data(DataPacket::FileCancel { transfer_id }));
                                let _ = reply.send(Ok(()));
                            }
                            Ok(false) => {
                                let _ = reply.send(Err(ConnectionError::State(format!(
                                    "transfer {} not found",
                                    transfer_id
                                ))));
                            }
                            Err(error) => {
                                let _ = reply.send(Err(ConnectionError::from(error)));
                            }
                        }
                    }
                    Some(SessionCommand::Shutdown) => break,
                    None => break,
                }
            }
        }
    }

    Ok(())
}

async fn handle_incoming_packet(
    lifecycle_tx: &mpsc::Sender<SessionLifecycleEvent>,
    session_id: SessionId,
    peer_noob_id: &str,
    peer_device_id: &str,
    packet: Packet,
    outbox: &mut PacketOutbox,
    sender: &mut FileSender,
    receiver: &mut FileReceiverStateMachine,
) -> Result<bool, ConnectionError> {
    match packet {
        Packet::Ping { timestamp } => {
            let _ = outbox.queue_control(Packet::Pong { timestamp });
            Ok(false)
        }
        Packet::Pong { .. } => Ok(true),
        Packet::Control(ControlPacket::Disconnect { .. }) => Ok(false),
        Packet::Control(_) => Ok(false),
        Packet::Handshake(_) => Err(ConnectionError::State(
            "unexpected handshake packet after session established".to_string(),
        )),
        Packet::Data(DataPacket::ClipboardText { event_id, content }) => {
            let _ = lifecycle_tx
                .send(SessionLifecycleEvent::Network(NetworkEvent::TextReceived {
                    session_id,
                    event_id,
                    content,
                    peer_noob_id: peer_noob_id.to_string(),
                    peer_device_id: peer_device_id.to_string(),
                }))
                .await;
            Ok(false)
        }
        Packet::Data(DataPacket::FileStart {
            transfer_id,
            file_name,
            file_size,
            total_chunks,
        }) => {
            let request = receiver
                .register_file_start(transfer_id, &file_name, file_size, total_chunks)
                .await?;
            let _ = lifecycle_tx
                .send(SessionLifecycleEvent::Network(NetworkEvent::IncomingTransferOffered {
                    offer: IncomingTransferOffer {
                        ticket: TransferTicket {
                            session_id,
                            raw_id: request.transfer_id,
                        },
                        session_id,
                        peer_noob_id: peer_noob_id.to_string(),
                        peer_device_id: peer_device_id.to_string(),
                        file_name: request.file_name,
                        file_size: request.file_size,
                        total_chunks: request.total_chunks,
                        offered_at_ms: now_millis_u64(),
                    },
                }))
                .await;
            Ok(false)
        }
        Packet::Data(DataPacket::FileDecision {
            transfer_id,
            accept,
            reason,
        }) => {
            sender.on_file_decision(transfer_id, accept, reason);
            Ok(false)
        }
        Packet::Data(DataPacket::FileChunk {
            transfer_id,
            seq,
            data,
        }) => {
            let progress = receiver.handle_file_chunk(transfer_id, seq, &data).await?;
            emit_transfer_updated(
                lifecycle_tx,
                session_id,
                peer_noob_id,
                peer_device_id,
                transfer_id,
                TransferDirection::Download,
                progress.done_bytes,
                progress.total_bytes,
                ActiveTransferState::InProgress,
            )
            .await;
            Ok(false)
        }
        Packet::Data(DataPacket::FileEnd {
            transfer_id,
            checksum,
        }) => {
            let downloaded = receiver.handle_file_end(transfer_id, &checksum).await?;
            emit_transfer_completed(
                lifecycle_tx,
                session_id,
                peer_noob_id,
                peer_device_id,
                transfer_id,
                TransferDirection::Download,
                TransferOutcome::Succeeded,
                Some(downloaded.path),
                None,
            )
            .await;
            Ok(false)
        }
        Packet::Data(DataPacket::FileCancel { transfer_id }) => {
            let _ = receiver.handle_file_cancel(transfer_id).await;
            emit_transfer_completed(
                lifecycle_tx,
                session_id,
                peer_noob_id,
                peer_device_id,
                transfer_id,
                TransferDirection::Download,
                TransferOutcome::Cancelled,
                None,
                Some("cancelled by remote peer".to_string()),
            )
            .await;
            Ok(false)
        }
    }
}

async fn emit_sender_update(
    lifecycle_tx: &mpsc::Sender<SessionLifecycleEvent>,
    session_id: SessionId,
    peer_noob_id: &str,
    peer_device_id: &str,
    update: TransferProgressEvent,
) {
    match update {
        TransferProgressEvent::Started {
            transfer_id,
            total_bytes,
        } => {
            emit_transfer_updated(
                lifecycle_tx,
                session_id,
                peer_noob_id,
                peer_device_id,
                transfer_id,
                TransferDirection::Upload,
                0,
                total_bytes,
                ActiveTransferState::Starting,
            )
            .await;
        }
        TransferProgressEvent::Progress {
            transfer_id,
            done_bytes,
            total_bytes,
        } => {
            emit_transfer_updated(
                lifecycle_tx,
                session_id,
                peer_noob_id,
                peer_device_id,
                transfer_id,
                TransferDirection::Upload,
                done_bytes,
                total_bytes,
                ActiveTransferState::InProgress,
            )
            .await;
        }
        TransferProgressEvent::Finished { transfer_id } => {
            emit_transfer_completed(
                lifecycle_tx,
                session_id,
                peer_noob_id,
                peer_device_id,
                transfer_id,
                TransferDirection::Upload,
                TransferOutcome::Succeeded,
                None,
                None,
            )
            .await;
        }
        TransferProgressEvent::Failed { transfer_id, reason } => {
            emit_transfer_completed(
                lifecycle_tx,
                session_id,
                peer_noob_id,
                peer_device_id,
                transfer_id,
                TransferDirection::Upload,
                TransferOutcome::Failed,
                None,
                Some(reason),
            )
            .await;
        }
        TransferProgressEvent::Rejected { transfer_id, reason } => {
            emit_transfer_completed(
                lifecycle_tx,
                session_id,
                peer_noob_id,
                peer_device_id,
                transfer_id,
                TransferDirection::Upload,
                TransferOutcome::Rejected,
                None,
                reason,
            )
            .await;
        }
        TransferProgressEvent::Cancelled { transfer_id, reason } => {
            emit_transfer_completed(
                lifecycle_tx,
                session_id,
                peer_noob_id,
                peer_device_id,
                transfer_id,
                TransferDirection::Upload,
                TransferOutcome::Cancelled,
                None,
                reason,
            )
            .await;
        }
    }
}

async fn emit_transfer_updated(
    lifecycle_tx: &mpsc::Sender<SessionLifecycleEvent>,
    session_id: SessionId,
    peer_noob_id: &str,
    peer_device_id: &str,
    transfer_id: u32,
    direction: TransferDirection,
    transferred_bytes: u64,
    file_size: u64,
    state: ActiveTransferState,
) {
    let _ = lifecycle_tx
        .send(SessionLifecycleEvent::Network(NetworkEvent::TransferUpdated {
            transfer: ActiveTransferInfo {
                ticket: TransferTicket {
                    session_id,
                    raw_id: transfer_id,
                },
                session_id,
                peer_noob_id: peer_noob_id.to_string(),
                peer_device_id: peer_device_id.to_string(),
                file_name: String::new(),
                file_size,
                transferred_bytes,
                direction,
                state,
                updated_at_ms: now_millis_u64(),
            },
        }))
        .await;
}

async fn emit_transfer_completed(
    lifecycle_tx: &mpsc::Sender<SessionLifecycleEvent>,
    session_id: SessionId,
    peer_noob_id: &str,
    peer_device_id: &str,
    transfer_id: u32,
    direction: TransferDirection,
    outcome: TransferOutcome,
    saved_path: Option<std::path::PathBuf>,
    message: Option<String>,
) {
    let _ = lifecycle_tx
        .send(SessionLifecycleEvent::Network(NetworkEvent::TransferCompleted {
            transfer: CompletedTransferInfo {
                ticket: TransferTicket {
                    session_id,
                    raw_id: transfer_id,
                },
                session_id,
                peer_noob_id: peer_noob_id.to_string(),
                peer_device_id: peer_device_id.to_string(),
                file_name: String::new(),
                file_size: 0,
                direction,
                outcome,
                saved_path,
                message,
                finished_at_ms: now_millis_u64(),
            },
        }))
        .await;
}

fn now_millis_u64() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
