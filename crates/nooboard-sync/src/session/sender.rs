use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use tokio::fs;
use tokio::io::AsyncReadExt;
use tracing::{debug, warn};

use crate::config::SyncConfig;
use crate::engine::TransferState;
use crate::error::ConnectionError;
use crate::protocol::{DataPacket, Packet};

use super::SessionResult;
use super::path::sanitize_file_name;

#[derive(Debug, Clone)]
pub struct TransferStateUpdate {
    pub transfer_id: u32,
    pub state: TransferState,
}

struct OutgoingTransfer {
    transfer_id: u32,
    total_chunks: u32,
    next_seq: u32,
    deadline: Instant,
    accepted: Option<bool>,
    decision_reason: Option<String>,
    file: fs::File,
    file_name: String,
    file_size: u64,
    sent_bytes: u64,
    chunk_size: usize,
    hasher: Sha256,
    end_sent: bool,
}

pub struct FileSender {
    pending_files: VecDeque<PathBuf>,
    upload: Option<OutgoingTransfer>,
    transfer_id_seed: u32,
    outbox: VecDeque<Packet>,
    updates: VecDeque<TransferStateUpdate>,
}

impl FileSender {
    pub fn new() -> Self {
        Self {
            pending_files: VecDeque::new(),
            upload: None,
            transfer_id_seed: 1,
            outbox: VecDeque::new(),
            updates: VecDeque::new(),
        }
    }

    pub fn enqueue_file(&mut self, path: PathBuf) {
        self.pending_files.push_back(path);
    }

    pub fn on_file_decision(&mut self, transfer_id: u32, accept: bool, reason: Option<String>) {
        if let Some(transfer) = self.upload.as_mut() {
            if transfer.transfer_id == transfer_id {
                transfer.accepted = Some(accept);
                transfer.decision_reason = reason.clone();
                if !accept {
                    warn!(
                        transfer_id,
                        reason = %reason.unwrap_or_else(|| "peer rejected".to_string()),
                        "peer rejected file transfer"
                    );
                }
            }
        }
    }

    pub fn pop_packet(&mut self) -> Option<Packet> {
        self.outbox.pop_front()
    }

    pub fn requeue_packet_front(&mut self, packet: Packet) {
        self.outbox.push_front(packet);
    }

    pub fn pop_update(&mut self) -> Option<TransferStateUpdate> {
        self.updates.pop_front()
    }

    pub async fn tick(
        &mut self,
        config: &SyncConfig,
        peer_node_id: &str,
        allow_new_data: bool,
    ) -> SessionResult<()> {
        if self.upload.is_none() && allow_new_data {
            if let Some(path) = self.pending_files.pop_front() {
                match self
                    .start_upload(config, &path, self.transfer_id_seed, peer_node_id)
                    .await
                {
                    Ok(()) => {
                        self.transfer_id_seed = self.transfer_id_seed.wrapping_add(1);
                    }
                    Err(error) => {
                        warn!(
                            peer = %peer_node_id,
                            path = %path.display(),
                            "skip file upload: {error}"
                        );
                        self.updates.push_back(TransferStateUpdate {
                            transfer_id: self.transfer_id_seed,
                            state: TransferState::Failed {
                                reason: error.to_string(),
                            },
                        });
                        self.transfer_id_seed = self.transfer_id_seed.wrapping_add(1);
                    }
                }
            }
        }

        self.progress_upload(config, allow_new_data).await
    }

    async fn start_upload(
        &mut self,
        config: &SyncConfig,
        path: &Path,
        transfer_id: u32,
        peer_node_id: &str,
    ) -> SessionResult<()> {
        let metadata = fs::metadata(path).await.map_err(ConnectionError::Io)?;
        if !metadata.is_file() {
            return Err(ConnectionError::State(format!(
                "{} is not a regular file",
                path.display()
            )));
        }

        if metadata.len() > config.max_file_size {
            return Err(ConnectionError::State(format!(
                "{} exceeds max file size",
                path.display()
            )));
        }

        let raw_file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                ConnectionError::State(format!("invalid file name: {}", path.display()))
            })?;
        let file_name = sanitize_file_name(raw_file_name).map_err(|_| {
            ConnectionError::State(format!("invalid file name: {}", path.display()))
        })?;

        let total_chunks = if metadata.len() == 0 {
            0
        } else {
            metadata.len().div_ceil(config.file_chunk_size as u64) as u32
        };

        let file = fs::File::open(path).await.map_err(ConnectionError::Io)?;
        self.outbox.push_back(Packet::Data(DataPacket::FileStart {
            transfer_id,
            file_name: file_name.clone(),
            file_size: metadata.len(),
            total_chunks,
        }));
        self.updates.push_back(TransferStateUpdate {
            transfer_id,
            state: TransferState::Started {
                file_name: file_name.clone(),
                total_bytes: metadata.len(),
            },
        });

        debug!(
            peer = %peer_node_id,
            transfer_id,
            path = %path.display(),
            "created outgoing transfer, waiting for decision"
        );

        self.upload = Some(OutgoingTransfer {
            transfer_id,
            total_chunks,
            next_seq: 0,
            deadline: Instant::now() + Duration::from_millis(config.file_decision_timeout_ms),
            accepted: None,
            decision_reason: None,
            file,
            file_name,
            file_size: metadata.len(),
            sent_bytes: 0,
            chunk_size: config.file_chunk_size,
            hasher: Sha256::new(),
            end_sent: false,
        });

        Ok(())
    }

    async fn progress_upload(
        &mut self,
        config: &SyncConfig,
        allow_new_data: bool,
    ) -> SessionResult<()> {
        let (outbox, upload) = (&mut self.outbox, &mut self.upload);
        let Some(state) = upload.as_mut() else {
            return Ok(());
        };

        match state.accepted {
            Some(false) => {
                outbox.push_back(Packet::Data(DataPacket::FileCancel {
                    transfer_id: state.transfer_id,
                }));
                self.updates.push_back(TransferStateUpdate {
                    transfer_id: state.transfer_id,
                    state: TransferState::Cancelled {
                        reason: state.decision_reason.take(),
                    },
                });
                *upload = None;
                return Ok(());
            }
            None if Instant::now() > state.deadline => {
                outbox.push_back(Packet::Data(DataPacket::FileCancel {
                    transfer_id: state.transfer_id,
                }));
                self.updates.push_back(TransferStateUpdate {
                    transfer_id: state.transfer_id,
                    state: TransferState::Failed {
                        reason: format!(
                            "file decision timeout for outgoing transfer {}",
                            state.transfer_id
                        ),
                    },
                });
                *upload = None;
                return Ok(());
            }
            None => {
                return Ok(());
            }
            Some(true) => {}
        }

        if !allow_new_data {
            return Ok(());
        }

        if state.end_sent {
            *upload = None;
            return Ok(());
        }

        if state.next_seq >= state.total_chunks {
            let checksum = hex::encode(state.hasher.clone().finalize());
            outbox.push_back(Packet::Data(DataPacket::FileEnd {
                transfer_id: state.transfer_id,
                checksum,
            }));
            state.end_sent = true;
            self.updates.push_back(TransferStateUpdate {
                transfer_id: state.transfer_id,
                state: TransferState::Finished { path: None },
            });
            return Ok(());
        }

        let mut buffer = vec![0_u8; state.chunk_size.min(config.max_packet_size)];
        let read_size = state
            .file
            .read(&mut buffer)
            .await
            .map_err(ConnectionError::Io)?;
        if read_size == 0 {
            if state.file_size == 0 {
                outbox.push_back(Packet::Data(DataPacket::FileEnd {
                    transfer_id: state.transfer_id,
                    checksum: hex::encode(state.hasher.clone().finalize()),
                }));
                state.end_sent = true;
                self.updates.push_back(TransferStateUpdate {
                    transfer_id: state.transfer_id,
                    state: TransferState::Finished { path: None },
                });
                return Ok(());
            }

            outbox.push_back(Packet::Data(DataPacket::FileCancel {
                transfer_id: state.transfer_id,
            }));
            self.updates.push_back(TransferStateUpdate {
                transfer_id: state.transfer_id,
                state: TransferState::Failed {
                    reason: format!(
                        "unexpected EOF for transfer {} ({})",
                        state.transfer_id, state.file_name
                    ),
                },
            });
            *upload = None;
            return Ok(());
        }

        buffer.truncate(read_size);
        state.hasher.update(&buffer);
        state.sent_bytes = state
            .sent_bytes
            .saturating_add(u64::try_from(read_size).unwrap_or(u64::MAX));

        outbox.push_back(Packet::Data(DataPacket::FileChunk {
            transfer_id: state.transfer_id,
            seq: state.next_seq,
            data: buffer,
        }));
        self.updates.push_back(TransferStateUpdate {
            transfer_id: state.transfer_id,
            state: TransferState::Progress {
                done_bytes: state.sent_bytes,
                total_bytes: state.file_size,
                bps: None,
                eta_ms: None,
            },
        });

        state.next_seq = state.next_seq.saturating_add(1);
        tokio::task::yield_now().await;

        Ok(())
    }
}
