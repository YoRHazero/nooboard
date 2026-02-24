use std::collections::HashSet;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tokio::sync::{broadcast, mpsc};
use tokio::time::{MissedTickBehavior, interval};
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use tracing::{debug, warn};
use uuid::Uuid;

use crate::config::SyncConfig;
use crate::engine::SyncEvent;
use crate::protocol::{DataPacket, Packet};

use super::ConnectionResult;
use super::receiver::{FileReceiverLimits, FileReceiverStateMachine, IdleTimeoutAction};
use super::sender::FileSender;
use super::stream::PriorityPacketStream;

#[derive(Debug)]
pub enum ConnectionCommand {
    SendText(String),
    SendFile(std::path::PathBuf),
    FileDecision {
        transfer_id: u32,
        accept: bool,
        reason: Option<String>,
    },
    Shutdown,
}

pub struct ConnectionActorContext {
    pub peer_node_id: String,
    pub config: SyncConfig,
    pub framed: Framed<tokio_rustls::TlsStream<tokio::net::TcpStream>, LengthDelimitedCodec>,
    pub command_rx: mpsc::Receiver<ConnectionCommand>,
    pub event_tx: mpsc::Sender<SyncEvent>,
    pub shutdown_rx: broadcast::Receiver<()>,
}

pub async fn run_connection_actor(mut ctx: ConnectionActorContext) -> ConnectionResult<()> {
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

        sender.tick(&ctx.config, &ctx.peer_node_id).await?;
        while let Some(packet) = sender.pop_packet() {
            stream.queue_data(packet);
        }

        if stream.flush_one().await? {
            continue;
        }

        tokio::select! {
            _ = ctx.shutdown_rx.recv() => {
                debug!(peer=%ctx.peer_node_id, "connection actor received shutdown");
                break;
            }
            _ = ping_timer.tick() => {
                stream.queue_control(Packet::Ping { timestamp: now_millis_u64() });
            }
            _ = idle_timer.tick() => {
                let actions = receiver
                    .collect_idle_actions(Duration::from_millis(ctx.config.transfer_idle_timeout_ms))
                    .await?;
                for action in actions {
                    match action {
                        IdleTimeoutAction::RejectDecision { transfer_id, reason } => {
                            stream.queue_data(Packet::Data(DataPacket::FileDecision {
                                transfer_id,
                                accept: false,
                                reason: Some(reason),
                            }));
                        }
                        IdleTimeoutAction::CancelTransfer { transfer_id, reason } => {
                            warn!(peer=%ctx.peer_node_id, transfer_id, "{reason}");
                            stream.queue_data(Packet::Data(DataPacket::FileCancel { transfer_id }));
                        }
                    }
                }
            }
            maybe_command = ctx.command_rx.recv() => {
                match maybe_command {
                    Some(ConnectionCommand::SendText(content)) => {
                        stream.queue_data(Packet::Data(DataPacket::ClipboardText {
                            id: Uuid::now_v7().to_string(),
                            content,
                        }));
                    }
                    Some(ConnectionCommand::SendFile(path)) => {
                        sender.enqueue_file(path);
                    }
                    Some(ConnectionCommand::FileDecision { transfer_id, accept, reason }) => {
                        match receiver.apply_decision(transfer_id, accept).await {
                            Ok(()) => {
                                stream.queue_data(Packet::Data(DataPacket::FileDecision {
                                    transfer_id,
                                    accept,
                                    reason,
                                }));
                            }
                            Err(error) => {
                                warn!(peer=%ctx.peer_node_id, transfer_id, "invalid local file decision: {error}");
                            }
                        }
                    }
                    Some(ConnectionCommand::Shutdown) | None => break,
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
                        ).await? {
                            last_pong = Instant::now();
                        }
                    }
                    None => break,
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
    peer_node_id: &str,
    packet: Packet,
    stream: &mut PriorityPacketStream,
    sender: &mut FileSender,
    receiver: &mut FileReceiverStateMachine,
    seen_text_ids: &mut HashSet<String>,
    event_tx: &mpsc::Sender<SyncEvent>,
) -> ConnectionResult<bool> {
    match packet {
        Packet::Handshake(_) => {
            return Err(crate::error::ConnectionError::State(
                "received handshake packet after authentication".to_string(),
            ));
        }
        Packet::Ping { timestamp } => {
            stream.queue_control(Packet::Pong { timestamp });
            return Ok(true);
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
                    }
                    Err(error) => {
                        stream.queue_data(Packet::Data(DataPacket::FileDecision {
                            transfer_id,
                            accept: false,
                            reason: Some(error.to_string()),
                        }));
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
            } => {
                if let Err(error) = receiver.handle_file_chunk(transfer_id, seq, &data).await {
                    warn!(transfer_id, "file chunk failed: {error}");
                    stream.queue_data(Packet::Data(DataPacket::FileCancel { transfer_id }));
                    let _ = receiver.abort_transfer(transfer_id).await;
                }
            }
            DataPacket::FileEnd {
                transfer_id,
                checksum,
            } => match receiver.handle_file_end(transfer_id, &checksum).await {
                Ok(downloaded) => {
                    let _ = event_tx
                        .send(SyncEvent::FileDownloaded {
                            path: downloaded.path,
                            size: downloaded.size,
                        })
                        .await;
                }
                Err(error) => {
                    warn!(transfer_id, "file end failed: {error}");
                    stream.queue_data(Packet::Data(DataPacket::FileCancel { transfer_id }));
                    let _ = receiver.abort_transfer(transfer_id).await;
                }
            },
            DataPacket::FileCancel { transfer_id } => {
                let _ = receiver.handle_file_cancel(transfer_id).await;
            }
        },
    }

    Ok(true)
}
