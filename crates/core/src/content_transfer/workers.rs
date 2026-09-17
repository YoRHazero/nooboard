use super::{ContentStage, ContentTransfers};
use crate::{link::queue::Outbox, ports::ClipboardPort};
use nooboard_clipboard::{Content, ImageData, ImageEncoding, Snapshot};
use nooboard_network::{
    ContentKind, ContentResult, FileEntry, Manifest, Message, MessageId, TransferError,
};
use nooboard_storage::files::{FileSpec, IncomingBatch, PreparedBatch};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::{
    io::AsyncReadExt,
    sync::{Semaphore, mpsc},
};

pub(crate) enum Source {
    Clipboard(u64),
    Files(Vec<PathBuf>),
}
pub(crate) enum WorkerEvent {
    Prepared {
        id: MessageId,
        result: Result<(ContentKind, Arc<PreparedBatch>), TransferError>,
    },
    Preparing {
        id: MessageId,
        done: u64,
        total: u64,
    },
    Progress {
        key: String,
        done: u64,
    },
    Stage {
        key: String,
        stage: ContentStage,
    },
    Finished {
        key: String,
        stage: ContentStage,
        error: Option<TransferError>,
    },
    Ready {
        key: String,
        transaction: IncomingBatch,
        image: Option<ImageData>,
    },
    Saved {
        key: String,
        result: Result<Vec<PathBuf>, TransferError>,
        image: Option<ImageData>,
    },
    Applied {
        key: String,
        result: nooboard_clipboard::Result<Snapshot>,
    },
    Preview {
        revision: u64,
        result: nooboard_clipboard::Result<(Vec<u8>, u32, u32)>,
    },
}
pub(crate) fn storage_error(error: nooboard_storage::Error) -> TransferError {
    match error {
        nooboard_storage::Error::Cancelled => TransferError::Cancelled,
        nooboard_storage::Error::TooLarge => TransferError::TooLarge,
        nooboard_storage::Error::SourceChanged => TransferError::SourceChanged,
        nooboard_storage::Error::Integrity => TransferError::Integrity,
        nooboard_storage::Error::InvalidArgument(_) => TransferError::Unsupported,
        _ => TransferError::Io,
    }
}
pub(crate) fn prepare(
    id: MessageId,
    source: Source,
    clipboard: Arc<dyn ClipboardPort>,
    cancel: Arc<AtomicBool>,
    events: mpsc::Sender<WorkerEvent>,
    image_limit: Arc<Semaphore>,
) {
    tokio::spawn(async move {
        let content = match source {
            Source::Files(paths) => Ok(Content::Files(paths)),
            Source::Clipboard(revision) => match clipboard.read().await {
                Ok(snapshot) if snapshot.revision == revision => Ok(snapshot.content),
                Ok(_) => Err(TransferError::SourceChanged),
                Err(_) => Err(TransferError::Clipboard),
            },
        };
        let update = events.clone();
        let batch_id = id.clone();
        let permit = if matches!(&content, Ok(Content::Image(_))) {
            tokio::select! {
                permit = image_limit.acquire_owned() => permit.ok(),
                _ = cancelled(cancel.clone()) => None,
            }
        } else {
            None
        };
        let result = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            if cancel.load(Ordering::Acquire) {
                return Err(TransferError::Cancelled);
            }
            match content? {
                Content::Files(paths) => {
                    let mut last = Instant::now() - Duration::from_secs(1);
                    PreparedBatch::from_paths(&paths, &cancel, |done, total| {
                        if last.elapsed() >= Duration::from_millis(100) || done == total {
                            let _ = update.try_send(WorkerEvent::Preparing {
                                id: batch_id.clone(),
                                done,
                                total,
                            });
                            last = Instant::now();
                        }
                    })
                    .map(|batch| (ContentKind::Files, Arc::new(batch)))
                    .map_err(storage_error)
                }
                Content::Image(image) => {
                    let png = image.png().map_err(|_| TransferError::Unsupported)?;
                    if cancel.load(Ordering::Acquire) {
                        return Err(TransferError::Cancelled);
                    }
                    PreparedBatch::from_bytes("image.png", &png.bytes)
                        .map(|b| (ContentKind::Image, Arc::new(b)))
                        .map_err(storage_error)
                }
                _ => Err(TransferError::Unsupported),
            }
        })
        .await
        .unwrap_or(Err(TransferError::Io));
        let _ = events.send(WorkerEvent::Prepared { id, result }).await;
    });
}

pub(crate) struct SenderJob {
    pub key: String,
    pub id: MessageId,
    pub epoch: u64,
    pub kind: ContentKind,
    pub data: Arc<PreparedBatch>,
    pub outbox: Outbox,
    pub lane: Arc<Semaphore>,
    pub messages: mpsc::Receiver<Message>,
    pub events: mpsc::Sender<WorkerEvent>,
    pub cancel: Arc<AtomicBool>,
}
impl SenderJob {
    pub async fn run(mut self) {
        let result = self.send().await;
        match result {
            Ok((stage, error)) => {
                let _ = self
                    .events
                    .send(WorkerEvent::Finished {
                        key: self.key,
                        stage,
                        error,
                    })
                    .await;
            }
            Err(error) => {
                self.outbox.cancel_bulk(&self.id);
                let _ = self.outbox.control(Message::Cancel {
                    id: self.id.clone(),
                });
                let _ = self
                    .events
                    .send(WorkerEvent::Finished {
                        key: self.key,
                        stage: if error == TransferError::Timeout || error == TransferError::Offline
                        {
                            ContentStage::Unconfirmed
                        } else {
                            ContentStage::Failed
                        },
                        error: Some(error),
                    })
                    .await;
            }
        }
    }
    async fn receive(&mut self) -> Result<Message, TransferError> {
        tokio::time::timeout(Duration::from_secs(60), self.messages.recv())
            .await
            .map_err(|_| TransferError::Timeout)?
            .ok_or(TransferError::Offline)
    }
    async fn stage(&self, stage: ContentStage) {
        let _ = self
            .events
            .send(WorkerEvent::Stage {
                key: self.key.clone(),
                stage,
            })
            .await;
    }
    async fn send(&mut self) -> Result<(ContentStage, Option<TransferError>), TransferError> {
        let _permit = tokio::select! {
            permit = self.lane.clone().acquire_owned() => permit.map_err(|_| TransferError::Offline)?,
            _ = cancelled(self.cancel.clone()) => return Ok((ContentStage::Cancelled, Some(TransferError::Cancelled))),
        };
        if self.cancel.load(Ordering::Acquire) {
            return Ok((ContentStage::Cancelled, Some(TransferError::Cancelled)));
        }
        let manifest = Manifest {
            kind: self.kind,
            files: self
                .data
                .files
                .iter()
                .map(|f| FileEntry {
                    name: f.name.clone(),
                    bytes: f.bytes,
                    sha256: f.sha256,
                })
                .collect(),
        };
        self.outbox
            .control(Message::Offer {
                id: self.id.clone(),
                target_epoch: self.epoch,
                manifest,
            })
            .map_err(|_| TransferError::Busy)?;
        self.stage(ContentStage::Waiting).await;
        match self.receive().await? {
            Message::Accept { .. } => {}
            Message::Outcome { result, error, .. } => return before_finish(result, error),
            Message::Cancel { .. } => {
                return Ok((ContentStage::Cancelled, Some(TransferError::Cancelled)));
            }
            _ => return Err(TransferError::Protocol),
        }
        if self.cancel.load(Ordering::Acquire) {
            return self.cancel_wait().await;
        }
        self.stage(ContentStage::Sending).await;
        let mut done = 0;
        let mut last = Instant::now() - Duration::from_secs(1);
        for index in 0..self.data.files.len() {
            let data = self.data.clone();
            let file = tokio::task::spawn_blocking(move || data.open(index))
                .await
                .map_err(|_| TransferError::Io)?
                .map_err(storage_error)?;
            let mut file = tokio::fs::File::from_std(file);
            let size = self.data.files[index].bytes;
            let mut offset = 0;
            while offset < size {
                if self.cancel.load(Ordering::Acquire) {
                    return self.cancel_wait().await;
                }
                let mut bytes =
                    vec![0; ((size - offset) as usize).min(nooboard_network::MAX_CHUNK_BYTES)];
                file.read_exact(&mut bytes)
                    .await
                    .map_err(|_| TransferError::Io)?;
                let end = offset + bytes.len() as u64;
                tokio::time::timeout(
                    Duration::from_secs(60),
                    self.outbox.bulk(Message::Chunk {
                        id: self.id.clone(),
                        file: index as u16,
                        offset,
                        bytes,
                    }),
                )
                .await
                .map_err(|_| TransferError::Timeout)?;
                match self.receive().await? {
                    Message::ChunkAck { file, offset, .. }
                        if file == index as u16 && offset == end => {}
                    Message::Outcome { result, error, .. } => return before_finish(result, error),
                    Message::Cancel { .. } => {
                        return Ok((ContentStage::Cancelled, Some(TransferError::Cancelled)));
                    }
                    _ => return Err(TransferError::Protocol),
                }
                done += end - offset;
                offset = end;
                if last.elapsed() >= Duration::from_millis(100) || done == self.data.bytes() {
                    let _ = self.events.try_send(WorkerEvent::Progress {
                        key: self.key.clone(),
                        done,
                    });
                    last = Instant::now();
                }
            }
        }
        if self.cancel.load(Ordering::Acquire) {
            return self.cancel_wait().await;
        }
        self.outbox
            .control(Message::Finish {
                id: self.id.clone(),
            })
            .map_err(|_| TransferError::Busy)?;
        self.stage(ContentStage::Verifying).await;
        match self.receive().await? {
            Message::Outcome { result, error, .. } => Ok(outcome(result, error)),
            Message::Cancel { .. } => Ok((ContentStage::Cancelled, Some(TransferError::Cancelled))),
            _ => Err(TransferError::Protocol),
        }
    }
    async fn cancel_wait(
        &mut self,
    ) -> Result<(ContentStage, Option<TransferError>), TransferError> {
        self.outbox.cancel_bulk(&self.id);
        let _ = self.outbox.control(Message::Cancel {
            id: self.id.clone(),
        });
        loop {
            match self.receive().await? {
                Message::Outcome { result, error, .. } => return before_finish(result, error),
                Message::Cancel { .. } => {
                    return Ok((ContentStage::Cancelled, Some(TransferError::Cancelled)));
                }
                Message::ChunkAck { .. } | Message::Accept { .. } => {}
                _ => return Err(TransferError::Protocol),
            }
        }
    }
}
fn before_finish(
    result: ContentResult,
    error: Option<TransferError>,
) -> Result<(ContentStage, Option<TransferError>), TransferError> {
    if matches!(result, ContentResult::Applied | ContentResult::Saved) {
        Err(TransferError::Protocol)
    } else {
        Ok(outcome(result, error))
    }
}
fn outcome(
    result: ContentResult,
    error: Option<TransferError>,
) -> (ContentStage, Option<TransferError>) {
    (
        match result {
            ContentResult::Applied => ContentStage::Completed,
            ContentResult::Saved => ContentStage::Saved,
            ContentResult::Cancelled => ContentStage::Cancelled,
            _ => ContentStage::Failed,
        },
        error,
    )
}
async fn cancelled(cancel: Arc<AtomicBool>) {
    while !cancel.load(Ordering::Acquire) {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

pub(crate) struct ReceiverJob {
    pub peer: String,
    pub id: MessageId,
    pub directory: PathBuf,
    pub manifest: Manifest,
    pub outbox: Outbox,
    pub messages: mpsc::Receiver<Message>,
    pub events: mpsc::Sender<WorkerEvent>,
    pub cancel: Arc<AtomicBool>,
    pub image_limit: Arc<Semaphore>,
}
impl ReceiverJob {
    pub async fn run(mut self) {
        let key = ContentTransfers::key(&self.peer, &self.id, true);
        if let Err(error) = self.receive(&key).await {
            let _ = self.outbox.control(Message::Outcome {
                id: self.id,
                result: ContentResult::Failed,
                error: Some(error),
            });
            let _ = self
                .events
                .send(WorkerEvent::Finished {
                    key,
                    stage: if error == TransferError::Cancelled {
                        ContentStage::Cancelled
                    } else {
                        ContentStage::Failed
                    },
                    error: Some(error),
                })
                .await;
        }
    }
    async fn receive(&mut self, key: &str) -> Result<(), TransferError> {
        let specs = self
            .manifest
            .files
            .iter()
            .map(|f| FileSpec {
                name: f.name.clone(),
                bytes: f.bytes,
                sha256: f.sha256,
            })
            .collect();
        let root = self.directory.clone();
        let mut transaction =
            tokio::task::spawn_blocking(move || IncomingBatch::create(&root, specs))
                .await
                .map_err(|_| TransferError::Io)?
                .map_err(|_| TransferError::Directory)?;
        self.outbox
            .control(Message::Accept {
                id: self.id.clone(),
            })
            .map_err(|_| TransferError::Busy)?;
        let _ = self
            .events
            .send(WorkerEvent::Stage {
                key: key.into(),
                stage: ContentStage::Receiving,
            })
            .await;
        let mut done = 0;
        let mut last = Instant::now() - Duration::from_secs(1);
        loop {
            let message = tokio::time::timeout(Duration::from_secs(60), self.messages.recv())
                .await
                .map_err(|_| TransferError::Timeout)?
                .ok_or(TransferError::Offline)?;
            match message {
                Message::Chunk {
                    file,
                    offset,
                    bytes,
                    ..
                } => {
                    let count = bytes.len() as u64;
                    let cancel = self.cancel.clone();
                    transaction = tokio::task::spawn_blocking(move || {
                        nooboard_storage::files::check_cancelled(&cancel)?;
                        transaction.write(file as usize, offset, &bytes)?;
                        Ok::<_, nooboard_storage::Error>(transaction)
                    })
                    .await
                    .map_err(|_| TransferError::Io)?
                    .map_err(storage_error)?;
                    self.outbox
                        .control(Message::ChunkAck {
                            id: self.id.clone(),
                            file,
                            offset: offset + count,
                        })
                        .map_err(|_| TransferError::Busy)?;
                    done += count;
                    if last.elapsed() >= Duration::from_millis(100) || done == self.manifest.bytes()
                    {
                        let _ = self.events.try_send(WorkerEvent::Progress {
                            key: key.into(),
                            done,
                        });
                        last = Instant::now();
                    }
                }
                Message::Finish { .. } => {
                    let _ = self
                        .events
                        .send(WorkerEvent::Stage {
                            key: key.into(),
                            stage: ContentStage::Verifying,
                        })
                        .await;
                    let kind = self.manifest.kind;
                    let permit = if kind == ContentKind::Image {
                        Some(
                            self.image_limit
                                .clone()
                                .acquire_owned()
                                .await
                                .map_err(|_| TransferError::Offline)?,
                        )
                    } else {
                        None
                    };
                    let (transaction, image) = tokio::task::spawn_blocking(move || {
                        let _permit = permit;
                        transaction.complete().map_err(storage_error)?;
                        let image = if kind == ContentKind::Image {
                            let bytes = transaction
                                .read_staged(0, nooboard_clipboard::MAX_IMAGE_BYTES as u64)
                                .map_err(storage_error)?;
                            let image = ImageData::new(ImageEncoding::Png, bytes)
                                .map_err(|_| TransferError::Unsupported)?;
                            image.decode().map_err(|_| TransferError::Unsupported)?;
                            Some(image)
                        } else {
                            None
                        };
                        Ok::<_, TransferError>((transaction, image))
                    })
                    .await
                    .map_err(|_| TransferError::Io)??;
                    self.events
                        .send(WorkerEvent::Ready {
                            key: key.into(),
                            transaction,
                            image,
                        })
                        .await
                        .map_err(|_| TransferError::Offline)?;
                    return Ok(());
                }
                Message::Cancel { .. } => return Err(TransferError::Cancelled),
                _ => return Err(TransferError::Protocol),
            }
        }
    }
}
