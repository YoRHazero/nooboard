use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::error::FileReceiveError;

use super::path::{ensure_inside_download_dir, resolve_final_path, sanitize_file_name};

#[derive(Debug, Clone)]
pub struct FileReceiverLimits {
    pub download_dir: PathBuf,
    pub max_file_size: u64,
    pub active_downloads: usize,
}

#[derive(Debug, Clone)]
pub struct IncomingFileRequest {
    pub transfer_id: u32,
    pub file_name: String,
    pub file_size: u64,
    pub total_chunks: u32,
}

#[derive(Debug, Clone)]
pub struct DownloadedFile {
    pub path: PathBuf,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub enum IdleTimeoutAction {
    RejectDecision { transfer_id: u32, reason: String },
    CancelTransfer { transfer_id: u32, reason: String },
}

#[derive(Debug)]
enum TransferState {
    AwaitingDecision,
    Receiving {
        file: fs::File,
        written_chunks: u32,
        written_bytes: u64,
        hasher: Sha256,
    },
}

#[derive(Debug)]
struct IncomingTransfer {
    expected_size: u64,
    total_chunks: u32,
    tmp_path: PathBuf,
    final_path: PathBuf,
    last_activity: Instant,
    state: TransferState,
}

impl IncomingTransfer {
    async fn new(
        limits: &FileReceiverLimits,
        transfer_id: u32,
        file_name: &str,
        file_size: u64,
        total_chunks: u32,
    ) -> Result<(Self, IncomingFileRequest), FileReceiveError> {
        let sanitized_name = sanitize_file_name(file_name)?;
        let final_path = resolve_final_path(&limits.download_dir, &sanitized_name).await?;
        ensure_inside_download_dir(&limits.download_dir, &final_path)?;

        let tmp_path = limits
            .download_dir
            .join(format!("{}.{}.tmp", sanitized_name, transfer_id));
        ensure_inside_download_dir(&limits.download_dir, &tmp_path)?;

        let transfer = Self {
            expected_size: file_size,
            total_chunks,
            tmp_path,
            final_path,
            last_activity: Instant::now(),
            state: TransferState::AwaitingDecision,
        };

        let request = IncomingFileRequest {
            transfer_id,
            file_name: sanitized_name,
            file_size,
            total_chunks,
        };

        Ok((transfer, request))
    }

    fn is_awaiting_decision(&self) -> bool {
        matches!(self.state, TransferState::AwaitingDecision)
    }

    fn is_idle(&self, now: Instant, timeout: Duration) -> bool {
        now.duration_since(self.last_activity) > timeout
    }

    async fn accept(&mut self, transfer_id: u32) -> Result<(), FileReceiveError> {
        if !self.is_awaiting_decision() {
            return Err(FileReceiveError::DecisionAlreadyMade(transfer_id));
        }

        let file = fs::File::create(&self.tmp_path).await?;
        self.last_activity = Instant::now();
        self.state = TransferState::Receiving {
            file,
            written_chunks: 0,
            written_bytes: 0,
            hasher: Sha256::new(),
        };

        Ok(())
    }

    async fn write_chunk(
        &mut self,
        transfer_id: u32,
        seq: u32,
        data: &[u8],
    ) -> Result<(), FileReceiveError> {
        self.last_activity = Instant::now();

        match &mut self.state {
            TransferState::AwaitingDecision => Err(FileReceiveError::DecisionRequired(transfer_id)),
            TransferState::Receiving {
                file,
                written_chunks,
                written_bytes,
                hasher,
            } => {
                if seq != *written_chunks {
                    return Err(FileReceiveError::OutOfOrderChunk {
                        transfer_id,
                        expected: *written_chunks,
                        got: seq,
                    });
                }

                file.write_all(data).await?;
                hasher.update(data);
                *written_chunks = written_chunks.saturating_add(1);
                *written_bytes =
                    written_bytes.saturating_add(u64::try_from(data.len()).unwrap_or(u64::MAX));

                if *written_bytes > self.expected_size {
                    return Err(FileReceiveError::FileTooLarge {
                        size: *written_bytes,
                        max: self.expected_size,
                    });
                }

                Ok(())
            }
        }
    }

    async fn finish(
        self,
        transfer_id: u32,
        checksum: &str,
    ) -> Result<DownloadedFile, FileReceiveError> {
        let (mut file, written_chunks, written_bytes, hasher) = match self.state {
            TransferState::AwaitingDecision => {
                Self::remove_tmp_best_effort(&self.tmp_path).await;
                return Err(FileReceiveError::DecisionRequired(transfer_id));
            }
            TransferState::Receiving {
                file,
                written_chunks,
                written_bytes,
                hasher,
            } => (file, written_chunks, written_bytes, hasher),
        };

        file.flush().await?;
        file.sync_all().await?;
        drop(file);

        if written_bytes != self.expected_size {
            Self::remove_tmp_best_effort(&self.tmp_path).await;
            return Err(FileReceiveError::SizeMismatch {
                expected: self.expected_size,
                actual: written_bytes,
            });
        }

        if written_chunks != self.total_chunks {
            Self::remove_tmp_best_effort(&self.tmp_path).await;
            return Err(FileReceiveError::ChunkCountMismatch {
                expected: self.total_chunks,
                actual: written_chunks,
            });
        }

        let actual_checksum = hex::encode(hasher.finalize());
        if !checksum.is_empty() && checksum != actual_checksum {
            Self::remove_tmp_best_effort(&self.tmp_path).await;
            return Err(FileReceiveError::ChecksumMismatch {
                expected: checksum.to_string(),
                actual: actual_checksum,
            });
        }

        fs::rename(&self.tmp_path, &self.final_path).await?;

        Ok(DownloadedFile {
            path: self.final_path,
            size: written_bytes,
        })
    }

    async fn cleanup_tmp(self) -> Result<(), FileReceiveError> {
        if let TransferState::Receiving { file, .. } = self.state {
            drop(file);
        }

        match fs::remove_file(&self.tmp_path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(FileReceiveError::Io(error)),
        }
    }

    async fn remove_tmp_best_effort(path: &PathBuf) {
        let _ = fs::remove_file(path).await;
    }
}

#[derive(Debug)]
pub struct FileReceiverStateMachine {
    limits: FileReceiverLimits,
    active: HashMap<u32, IncomingTransfer>,
}

impl FileReceiverStateMachine {
    pub fn new(limits: FileReceiverLimits) -> Self {
        Self {
            limits,
            active: HashMap::new(),
        }
    }

    pub async fn register_file_start(
        &mut self,
        transfer_id: u32,
        file_name: &str,
        file_size: u64,
        total_chunks: u32,
    ) -> Result<IncomingFileRequest, FileReceiveError> {
        if self.active.contains_key(&transfer_id) {
            return Err(FileReceiveError::DuplicateTransfer(transfer_id));
        }

        if self.active.len() >= self.limits.active_downloads {
            return Err(FileReceiveError::TooManyActiveDownloads);
        }

        if file_size > self.limits.max_file_size {
            return Err(FileReceiveError::FileTooLarge {
                size: file_size,
                max: self.limits.max_file_size,
            });
        }

        let (transfer, request) = IncomingTransfer::new(
            &self.limits,
            transfer_id,
            file_name,
            file_size,
            total_chunks,
        )
        .await?;

        self.active.insert(transfer_id, transfer);
        Ok(request)
    }

    pub async fn apply_decision(
        &mut self,
        transfer_id: u32,
        accept: bool,
    ) -> Result<(), FileReceiveError> {
        if !self.active.contains_key(&transfer_id) {
            return Err(FileReceiveError::UnknownTransfer(transfer_id));
        }

        if !accept {
            self.abort_transfer(transfer_id).await?;
            return Ok(());
        }

        let transfer = self
            .active
            .get_mut(&transfer_id)
            .ok_or(FileReceiveError::UnknownTransfer(transfer_id))?;
        transfer.accept(transfer_id).await
    }

    pub async fn handle_file_chunk(
        &mut self,
        transfer_id: u32,
        seq: u32,
        data: &[u8],
    ) -> Result<(), FileReceiveError> {
        let result = {
            let transfer = self
                .active
                .get_mut(&transfer_id)
                .ok_or(FileReceiveError::UnknownTransfer(transfer_id))?;
            transfer.write_chunk(transfer_id, seq, data).await
        };

        if matches!(result, Err(FileReceiveError::FileTooLarge { .. })) {
            self.abort_transfer(transfer_id).await?;
        }

        result
    }

    pub async fn handle_file_end(
        &mut self,
        transfer_id: u32,
        checksum: &str,
    ) -> Result<DownloadedFile, FileReceiveError> {
        let transfer = self
            .active
            .remove(&transfer_id)
            .ok_or(FileReceiveError::UnknownTransfer(transfer_id))?;

        transfer.finish(transfer_id, checksum).await
    }

    pub async fn handle_file_cancel(&mut self, transfer_id: u32) -> Result<(), FileReceiveError> {
        self.abort_transfer(transfer_id).await
    }

    pub async fn abort_transfer(&mut self, transfer_id: u32) -> Result<(), FileReceiveError> {
        let Some(transfer) = self.active.remove(&transfer_id) else {
            return Ok(());
        };

        transfer.cleanup_tmp().await
    }

    pub async fn collect_idle_actions(
        &mut self,
        timeout: Duration,
    ) -> Result<Vec<IdleTimeoutAction>, FileReceiveError> {
        let now = Instant::now();
        let mut expired = Vec::new();

        for (transfer_id, transfer) in &self.active {
            if transfer.is_idle(now, timeout) {
                expired.push((*transfer_id, transfer.is_awaiting_decision()));
            }
        }

        let mut actions = Vec::with_capacity(expired.len());
        for (transfer_id, awaiting_decision) in expired {
            let reason = format!(
                "transfer {} idle for more than {}ms",
                transfer_id,
                timeout.as_millis()
            );
            self.abort_transfer(transfer_id).await?;

            if awaiting_decision {
                actions.push(IdleTimeoutAction::RejectDecision {
                    transfer_id,
                    reason,
                });
            } else {
                actions.push(IdleTimeoutAction::CancelTransfer {
                    transfer_id,
                    reason,
                });
            }
        }

        Ok(actions)
    }

    pub async fn cleanup_all(&mut self) {
        let transfer_ids: Vec<u32> = self.active.keys().copied().collect();
        for transfer_id in transfer_ids {
            let _ = self.abort_transfer(transfer_id).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::fs::ReadDir;

    async fn count_entries(mut read_dir: ReadDir) -> usize {
        let mut count = 0;
        while read_dir
            .next_entry()
            .await
            .expect("read_dir iteration should succeed")
            .is_some()
        {
            count += 1;
        }
        count
    }

    #[tokio::test]
    async fn abort_removes_tmp_file() {
        let dir = tempfile::tempdir().expect("tempdir must be created");
        let mut state = FileReceiverStateMachine::new(FileReceiverLimits {
            download_dir: dir.path().to_path_buf(),
            max_file_size: 10,
            active_downloads: 2,
        });

        state
            .register_file_start(7, "a.txt", 1, 1)
            .await
            .expect("register should succeed");
        state
            .apply_decision(7, true)
            .await
            .expect("accept should create tmp file");

        let tmp_files_before = count_entries(
            fs::read_dir(dir.path())
                .await
                .expect("read_dir should succeed"),
        )
        .await;
        assert_eq!(tmp_files_before, 1);

        state.abort_transfer(7).await.expect("abort should succeed");

        let tmp_files_after = count_entries(
            fs::read_dir(dir.path())
                .await
                .expect("read_dir should succeed"),
        )
        .await;
        assert_eq!(tmp_files_after, 0);
    }

    #[tokio::test]
    async fn idle_timeout_pending_transfer_rejects() {
        let dir = tempfile::tempdir().expect("tempdir must be created");
        let mut state = FileReceiverStateMachine::new(FileReceiverLimits {
            download_dir: dir.path().to_path_buf(),
            max_file_size: 10,
            active_downloads: 2,
        });

        state
            .register_file_start(9, "idle.txt", 1, 1)
            .await
            .expect("register should succeed");

        tokio::time::sleep(Duration::from_millis(5)).await;
        let actions = state
            .collect_idle_actions(Duration::from_millis(1))
            .await
            .expect("idle cleanup should succeed");

        assert!(matches!(
            actions.as_slice(),
            [IdleTimeoutAction::RejectDecision { transfer_id: 9, .. }]
        ));
    }

    #[tokio::test]
    async fn idle_timeout_receiving_transfer_cancels() {
        let dir = tempfile::tempdir().expect("tempdir must be created");
        let mut state = FileReceiverStateMachine::new(FileReceiverLimits {
            download_dir: dir.path().to_path_buf(),
            max_file_size: 10,
            active_downloads: 2,
        });

        state
            .register_file_start(10, "idle2.txt", 1, 1)
            .await
            .expect("register should succeed");
        state
            .apply_decision(10, true)
            .await
            .expect("accept should create tmp file");

        tokio::time::sleep(Duration::from_millis(5)).await;
        let actions = state
            .collect_idle_actions(Duration::from_millis(1))
            .await
            .expect("idle cleanup should succeed");

        assert!(matches!(
            actions.as_slice(),
            [IdleTimeoutAction::CancelTransfer {
                transfer_id: 10,
                ..
            }]
        ));

        let entries = count_entries(
            fs::read_dir(dir.path())
                .await
                .expect("read_dir should succeed"),
        )
        .await;
        assert_eq!(entries, 0);
    }
}
