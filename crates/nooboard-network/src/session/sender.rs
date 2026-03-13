use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use tokio::fs;
use tokio::io::AsyncReadExt;

use crate::config::NetworkConfig;
use crate::errors::ConnectionError;
use crate::protocol::{DataPacket, Packet};

#[derive(Debug, Clone)]
pub(crate) enum TransferProgressEvent {
    Started {
        transfer_id: u32,
        total_bytes: u64,
    },
    Progress {
        transfer_id: u32,
        done_bytes: u64,
        total_bytes: u64,
    },
    Finished {
        transfer_id: u32,
    },
    Failed {
        transfer_id: u32,
        reason: String,
    },
    Rejected {
        transfer_id: u32,
        reason: Option<String>,
    },
    Cancelled {
        transfer_id: u32,
        reason: Option<String>,
    },
}

#[derive(Debug)]
struct PendingFile {
    transfer_id: u32,
    path: PathBuf,
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

pub(crate) struct FileSender {
    pending_files: VecDeque<PendingFile>,
    upload: Option<OutgoingTransfer>,
    outbox: VecDeque<Packet>,
    updates: VecDeque<TransferProgressEvent>,
}

impl FileSender {
    pub(crate) fn new() -> Self {
        Self {
            pending_files: VecDeque::new(),
            upload: None,
            outbox: VecDeque::new(),
            updates: VecDeque::new(),
        }
    }

    pub(crate) fn enqueue_file(&mut self, transfer_id: u32, path: PathBuf) {
        self.pending_files
            .push_back(PendingFile { transfer_id, path });
    }

    pub(crate) fn cancel_transfer(&mut self, transfer_id: u32, reason: Option<String>) -> bool {
        if let Some(position) = self
            .pending_files
            .iter()
            .position(|pending| pending.transfer_id == transfer_id)
        {
            let _ = self.pending_files.remove(position);
            self.updates
                .push_back(TransferProgressEvent::Cancelled { transfer_id, reason });
            return true;
        }

        let Some(upload) = self.upload.as_ref() else {
            return false;
        };
        if upload.transfer_id != transfer_id || upload.end_sent {
            return false;
        }

        self.outbox
            .push_back(Packet::Data(DataPacket::FileCancel { transfer_id }));
        self.updates
            .push_back(TransferProgressEvent::Cancelled { transfer_id, reason });
        self.upload = None;
        true
    }

    pub(crate) fn on_file_decision(
        &mut self,
        transfer_id: u32,
        accept: bool,
        reason: Option<String>,
    ) {
        if let Some(transfer) = self.upload.as_mut() && transfer.transfer_id == transfer_id {
            transfer.accepted = Some(accept);
            transfer.decision_reason = reason;
        }
    }

    pub(crate) fn pop_packet(&mut self) -> Option<Packet> {
        self.outbox.pop_front()
    }

    pub(crate) fn requeue_packet_front(&mut self, packet: Packet) {
        self.outbox.push_front(packet);
    }

    pub(crate) fn pop_update(&mut self) -> Option<TransferProgressEvent> {
        self.updates.pop_front()
    }

    pub(crate) async fn tick(
        &mut self,
        config: &NetworkConfig,
        allow_new_data: bool,
    ) -> Result<(), ConnectionError> {
        if self.upload.is_none() && allow_new_data {
            if let Some(pending) = self.pending_files.pop_front() {
                match self.start_upload(config, &pending.path, pending.transfer_id).await {
                    Ok(()) => {}
                    Err(error) => {
                        self.updates.push_back(TransferProgressEvent::Failed {
                            transfer_id: pending.transfer_id,
                            reason: error.to_string(),
                        });
                    }
                }
            }
        }

        self.progress_upload(config, allow_new_data).await
    }

    async fn start_upload(
        &mut self,
        config: &NetworkConfig,
        path: &Path,
        transfer_id: u32,
    ) -> Result<(), ConnectionError> {
        let metadata = fs::metadata(path).await?;
        if !metadata.is_file() {
            return Err(ConnectionError::State(format!(
                "{} is not a regular file",
                path.display()
            )));
        }
        if metadata.len() > config.transfer.max_file_size {
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
        let file_name = super::path::sanitize_file_name(raw_file_name)
            .map_err(|_| ConnectionError::State(format!("invalid file name: {}", path.display())))?;

        let total_chunks = if metadata.len() == 0 {
            0
        } else {
            metadata.len().div_ceil(config.transfer.chunk_size as u64) as u32
        };

        let file = fs::File::open(path).await?;
        self.outbox.push_back(Packet::Data(DataPacket::FileStart {
            transfer_id,
            file_name: file_name.clone(),
            file_size: metadata.len(),
            total_chunks,
        }));
        self.updates.push_back(TransferProgressEvent::Started {
            transfer_id,
            total_bytes: metadata.len(),
        });

        self.upload = Some(OutgoingTransfer {
            transfer_id,
            total_chunks,
            next_seq: 0,
            deadline: Instant::now()
                + Duration::from_millis(config.transfer.decision_timeout_ms),
            accepted: None,
            decision_reason: None,
            file,
            file_name,
            file_size: metadata.len(),
            sent_bytes: 0,
            chunk_size: config.transfer.chunk_size,
            hasher: Sha256::new(),
            end_sent: false,
        });
        Ok(())
    }

    async fn progress_upload(
        &mut self,
        config: &NetworkConfig,
        allow_new_data: bool,
    ) -> Result<(), ConnectionError> {
        let (outbox, upload) = (&mut self.outbox, &mut self.upload);
        let Some(state) = upload.as_mut() else {
            return Ok(());
        };

        match state.accepted {
            Some(false) => {
                outbox.push_back(Packet::Data(DataPacket::FileCancel {
                    transfer_id: state.transfer_id,
                }));
                self.updates.push_back(TransferProgressEvent::Rejected {
                    transfer_id: state.transfer_id,
                    reason: state.decision_reason.take(),
                });
                *upload = None;
                return Ok(());
            }
            None if Instant::now() > state.deadline => {
                outbox.push_back(Packet::Data(DataPacket::FileCancel {
                    transfer_id: state.transfer_id,
                }));
                self.updates.push_back(TransferProgressEvent::Failed {
                    transfer_id: state.transfer_id,
                    reason: format!(
                        "file decision timeout for outgoing transfer {}",
                        state.transfer_id
                    ),
                });
                *upload = None;
                return Ok(());
            }
            None => return Ok(()),
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
            self.updates.push_back(TransferProgressEvent::Finished {
                transfer_id: state.transfer_id,
            });
            return Ok(());
        }

        let mut buffer = vec![0_u8; state.chunk_size.min(config.transport.max_packet_size)];
        let read_size = state.file.read(&mut buffer).await?;
        if read_size == 0 {
            if state.file_size == 0 {
                outbox.push_back(Packet::Data(DataPacket::FileEnd {
                    transfer_id: state.transfer_id,
                    checksum: hex::encode(state.hasher.clone().finalize()),
                }));
                state.end_sent = true;
                self.updates.push_back(TransferProgressEvent::Finished {
                    transfer_id: state.transfer_id,
                });
                return Ok(());
            }

            outbox.push_back(Packet::Data(DataPacket::FileCancel {
                transfer_id: state.transfer_id,
            }));
            self.updates.push_back(TransferProgressEvent::Failed {
                transfer_id: state.transfer_id,
                reason: format!(
                    "unexpected EOF for transfer {} ({})",
                    state.transfer_id, state.file_name
                ),
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
        self.updates.push_back(TransferProgressEvent::Progress {
            transfer_id: state.transfer_id,
            done_bytes: state.sent_bytes,
            total_bytes: state.file_size,
        });
        state.next_seq = state.next_seq.saturating_add(1);
        tokio::task::yield_now().await;
        Ok(())
    }
}
