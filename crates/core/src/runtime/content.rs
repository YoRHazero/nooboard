use super::Runtime;
use crate::{
    ContentStage, ContentTransfer, Error, Result,
    content_transfer::{
        Active, ContentTransfers,
        workers::{self, ReceiverJob, SenderJob, Source, WorkerEvent},
    },
};
use nooboard_clipboard::{ImageData, ImageEncoding, Payload, ReadState, Snapshot};
use nooboard_network::{ContentKind, ContentResult, Message, MessageId, TransferError};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::{Semaphore, mpsc};

impl Runtime {
    pub(super) fn start_content(&mut self, source: Source) -> Result<MessageId> {
        if self.config.settings.paused {
            return Err(Error::Paused);
        }
        let targets = self.config.manual_targets.clone();
        if targets.is_empty() {
            return Err(Error::NoTargets);
        }
        if !self.content.available()
            || self.content.preparing.len() >= 4
            || self
                .content
                .rows
                .iter()
                .filter(|r| r.stage.pending())
                .count()
                + targets.len()
                > 128
        {
            return Err(Error::Busy);
        }
        let (kind, names) = match &source {
            Source::Files(paths) if !paths.is_empty() && paths.len() <= 256 => (
                ContentKind::Files,
                paths
                    .iter()
                    .map(|p| {
                        p.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned()
                    })
                    .collect(),
            ),
            Source::Files(_) => return Err(Error::Ineligible),
            Source::Clipboard(_)
                if self.view.snapshot.current.kind == crate::ClipboardKind::Image =>
            {
                (ContentKind::Image, vec!["image.png".into()])
            }
            Source::Clipboard(_) => (ContentKind::Files, self.view.snapshot.current.files.clone()),
        };
        self.sequence = self.sequence.checked_add(1).ok_or(Error::Configuration)?;
        let id = MessageId {
            session: self.namespace.clone(),
            sequence: self.sequence,
        };
        for peer in targets {
            let key = ContentTransfers::key(&peer, &id, false);
            let session = self
                .peers
                .get(&peer)
                .filter(|s| s.accepting && s.peer_epoch.is_some())
                .and_then(|s| s.session.as_ref().map(|link| (s, link)));
            let online = session.is_some();
            if let Some((state, link)) = session {
                let (messages, waiting) = mpsc::channel(8);
                self.content.active.insert(
                    key.clone(),
                    Active {
                        generation: state.generation,
                        epoch: state.peer_epoch.unwrap(),
                        outbox: link.outbox.clone(),
                        cancel: Arc::new(AtomicBool::new(false)),
                        messages,
                        waiting: Some(waiting),
                        task: None,
                    },
                );
            }
            self.content.rows.push_front(ContentTransfer {
                key,
                id: id.clone(),
                peer: peer.clone(),
                device_name: self.config.peers[&peer].device_name.clone(),
                incoming: false,
                kind,
                names: names.clone(),
                total_bytes: 0,
                completed_bytes: 0,
                prepared_bytes: 0,
                stage: if online {
                    ContentStage::Preparing
                } else {
                    ContentStage::Failed
                },
                error: if online {
                    None
                } else {
                    Some(TransferError::Offline)
                },
                saved_paths: Vec::new(),
                at_ms: crate::history::now_ms(),
            });
        }
        if self
            .content
            .rows
            .iter()
            .any(|r| r.id == id && r.stage.pending())
        {
            let cancel = Arc::new(AtomicBool::new(false));
            self.content.preparing.insert(id.clone(), cancel.clone());
            workers::prepare(
                id.clone(),
                source,
                self.clipboard.clone(),
                cancel,
                self.content.events.clone(),
                self.content.image_limit.clone(),
            );
        }
        self.publish_snapshot();
        Ok(id)
    }
    pub(super) fn content_node(&mut self, key: &str, node: &str) {
        if let Some(task) = self.content.row(key).cloned() {
            if node == "finished" && task.stage.pending() {
                return;
            }
            self.view.content_node(&task, node);
        }
    }
    pub(super) fn cancel_content(&mut self, peer: Option<&str>, incoming_only: bool) {
        let keys: Vec<_> = self
            .content
            .rows
            .iter()
            .filter(|r| {
                peer.is_none_or(|p| r.peer == p)
                    && (!incoming_only || r.incoming)
                    && r.stage.cancellable()
            })
            .map(|r| r.key.clone())
            .collect();
        for key in keys {
            if self.content.cancel(&key)
                && self.content.row(&key).is_some_and(|r| !r.stage.pending())
            {
                self.content_node(&key, "finished");
            }
        }
    }
    pub(super) fn disconnect_content(&mut self, peer: &str) {
        let keys: Vec<_> = self
            .content
            .rows
            .iter()
            .filter(|r| {
                r.peer == peer
                    && r.stage.pending()
                    && !matches!(r.stage, ContentStage::Saving | ContentStage::Applying)
            })
            .map(|r| r.key.clone())
            .collect();
        for key in keys {
            let row = self.content.row_mut(&key).unwrap();
            row.stage = if row.incoming
                || matches!(row.stage, ContentStage::Preparing | ContentStage::Queued)
            {
                ContentStage::Failed
            } else {
                ContentStage::Unconfirmed
            };
            row.error = Some(TransferError::Offline);
            self.content.active.remove(&key);
            self.content_node(&key, "finished");
        }
        self.content.stop_unused_preparations();
    }
    pub(super) fn receive_content(&mut self, peer: &str, message: Message) -> Result<()> {
        if let Message::Offer {
            id,
            target_epoch,
            manifest,
        } = message
        {
            let key = ContentTransfers::key(peer, &id, true);
            if let Some(row) = self.content.row(&key) {
                if !row.stage.pending() {
                    self.content_receipt(&key);
                }
                return Ok(());
            }
            let reject = if !self.config.settings.accepting()
                || target_epoch != self.peers[peer].local_epoch
            {
                Some(TransferError::Denied)
            } else if self.config.settings.receive_directory.is_none() {
                Some(TransferError::Directory)
            } else if !self.content.available()
                || self
                    .content
                    .rows
                    .iter()
                    .any(|r| r.peer == peer && r.incoming && r.stage.pending())
            {
                Some(TransferError::Busy)
            } else {
                None
            };
            if let Some(error) = reject {
                self.control(
                    peer,
                    Message::Outcome {
                        id,
                        result: ContentResult::Rejected,
                        error: Some(error),
                    },
                );
                return Ok(());
            }
            manifest.validate()?;
            let state = &self.peers[peer];
            let outbox = state.session.as_ref().ok_or(Error::Offline)?.outbox.clone();
            let (messages, waiting) = mpsc::channel(8);
            let cancel = Arc::new(AtomicBool::new(false));
            let job = ReceiverJob {
                peer: peer.into(),
                id: id.clone(),
                directory: self.config.settings.receive_directory.clone().unwrap(),
                manifest: manifest.clone(),
                outbox: outbox.clone(),
                messages: waiting,
                events: self.content.events.clone(),
                cancel: cancel.clone(),
                image_limit: self.content.image_limit.clone(),
            };
            self.content.active.insert(
                key.clone(),
                Active {
                    generation: state.generation,
                    epoch: target_epoch,
                    outbox,
                    cancel,
                    messages,
                    waiting: None,
                    task: Some(tokio::spawn(job.run())),
                },
            );
            self.content.rows.push_front(ContentTransfer {
                key,
                id,
                peer: peer.into(),
                device_name: self.config.peers[peer].device_name.clone(),
                incoming: true,
                kind: manifest.kind,
                names: manifest.files.iter().map(|f| f.name.clone()).collect(),
                total_bytes: manifest.bytes(),
                completed_bytes: 0,
                prepared_bytes: 0,
                stage: ContentStage::Waiting,
                error: None,
                saved_paths: Vec::new(),
                at_ms: crate::history::now_ms(),
            });
            self.publish_snapshot();
            return Ok(());
        }
        let (id, incoming) = match &message {
            Message::Chunk { id, .. } | Message::Finish { id } => (id, true),
            Message::Accept { id } | Message::ChunkAck { id, .. } | Message::Outcome { id, .. } => {
                (id, false)
            }
            Message::Cancel { id } => {
                let key = ContentTransfers::key(peer, id, true);
                if self.content.row(&key).is_some() {
                    if self.content.cancel(&key) {
                        self.content_node(&key, "finished");
                        self.publish_snapshot();
                    } else if self.content.row(&key).is_some_and(|r| !r.stage.pending()) {
                        self.content_receipt(&key);
                    }
                    return Ok(());
                }
                (id, false)
            }
            _ => return Err(Error::Configuration),
        };
        let key = ContentTransfers::key(peer, id, incoming);
        if let Some(active) = self.content.active.get(&key) {
            if active.generation != self.peers[peer].generation {
                return Ok(());
            }
            if active.messages.try_send(message).is_err()
                && self
                    .content
                    .row(&key)
                    .is_some_and(|r| r.stage.cancellable())
            {
                return Err(Error::Busy);
            }
        }
        Ok(())
    }
    fn content_receipt(&mut self, key: &str) {
        if let Some(row) = self.content.row(key).cloned() {
            let result = match row.stage {
                ContentStage::Completed => ContentResult::Applied,
                ContentStage::Saved => ContentResult::Saved,
                ContentStage::Cancelled => ContentResult::Cancelled,
                _ => ContentResult::Failed,
            };
            self.control(
                &row.peer,
                Message::Outcome {
                    id: row.id,
                    result,
                    error: row.error,
                },
            );
        }
    }
    pub(super) async fn content_event(&mut self, event: WorkerEvent) {
        match event {
            WorkerEvent::Prepared { id, result } => {
                self.content.preparing.remove(&id);
                let keys: Vec<_> = self
                    .content
                    .rows
                    .iter()
                    .filter(|r| r.id == id && r.stage == ContentStage::Preparing)
                    .map(|r| r.key.clone())
                    .collect();
                for key in keys {
                    match &result {
                        Ok((kind, data)) => {
                            let row = self.content.row_mut(&key).unwrap();
                            row.kind = *kind;
                            row.total_bytes = data.bytes();
                            row.prepared_bytes = data.bytes();
                            row.names = data.files.iter().map(|f| f.name.clone()).collect();
                            row.stage = ContentStage::Queued;
                            let peer = row.peer.clone();
                            let lane = self
                                .content
                                .lanes
                                .entry(peer)
                                .or_insert_with(|| Arc::new(Semaphore::new(1)))
                                .clone();
                            if let Some(active) = self.content.active.get_mut(&key) {
                                let job = SenderJob {
                                    key: key.clone(),
                                    id: id.clone(),
                                    epoch: active.epoch,
                                    kind: *kind,
                                    data: data.clone(),
                                    outbox: active.outbox.clone(),
                                    lane,
                                    messages: active.waiting.take().unwrap(),
                                    events: self.content.events.clone(),
                                    cancel: active.cancel.clone(),
                                };
                                active.task = Some(tokio::spawn(job.run()));
                            }
                        }
                        Err(error) => {
                            let row = self.content.row_mut(&key).unwrap();
                            row.stage = ContentStage::Failed;
                            row.error = Some(*error);
                            self.content.active.remove(&key);
                            self.content_node(&key, "finished");
                        }
                    }
                }
            }
            WorkerEvent::Preparing { id, done, total } => {
                for row in self
                    .content
                    .rows
                    .iter_mut()
                    .filter(|r| r.id == id && r.stage == ContentStage::Preparing)
                {
                    row.prepared_bytes = done;
                    row.total_bytes = total;
                }
            }
            WorkerEvent::Progress { key, done } => {
                if let Some(row) = self.content.row_mut(&key).filter(|r| r.stage.pending()) {
                    row.completed_bytes = done.min(row.total_bytes);
                }
            }
            WorkerEvent::Stage { key, stage } => {
                if let Some(row) = self
                    .content
                    .row_mut(&key)
                    .filter(|r| r.stage.pending() && r.stage != ContentStage::Cancelling)
                {
                    let started = (row.stage == ContentStage::Queued
                        && stage == ContentStage::Waiting)
                        || (row.stage == ContentStage::Waiting && stage == ContentStage::Receiving);
                    row.stage = stage;
                    if started {
                        self.content_node(&key, "started");
                    }
                }
            }
            WorkerEvent::Finished { key, stage, error } => {
                if let Some(row) = self.content.row_mut(&key).filter(|r| r.stage.pending()) {
                    row.stage = stage;
                    row.error = error;
                    if matches!(stage, ContentStage::Completed | ContentStage::Saved) {
                        row.completed_bytes = row.total_bytes;
                    }
                    self.content.active.remove(&key);
                    self.content_node(&key, "finished");
                }
            }
            WorkerEvent::Ready {
                key,
                transaction,
                image,
            } => {
                let allowed = self
                    .content
                    .row(&key)
                    .is_some_and(|r| r.stage.cancellable())
                    && self.config.settings.accepting()
                    && self
                        .content
                        .active
                        .get(&key)
                        .is_some_and(|a| !a.cancel.load(Ordering::Acquire));
                if allowed {
                    self.content.row_mut(&key).unwrap().stage = ContentStage::Saving;
                    let events = self.content.events.clone();
                    tokio::spawn(async move {
                        let result = tokio::task::spawn_blocking(move || {
                            transaction.commit().map_err(workers::storage_error)
                        })
                        .await
                        .unwrap_or(Err(TransferError::Io));
                        let _ = events.send(WorkerEvent::Saved { key, result, image }).await;
                    });
                }
            }
            WorkerEvent::Saved { key, result, image } => match result {
                Ok(paths) => {
                    if let Some(row) = self.content.row_mut(&key) {
                        row.saved_paths = paths.clone();
                        row.completed_bytes = row.total_bytes;
                        row.stage = ContentStage::Applying;
                        self.apply_content(
                            key,
                            image.map(Payload::Image).unwrap_or(Payload::Files(paths)),
                        );
                    }
                }
                Err(error) => {
                    if let Some(row) = self.content.row_mut(&key) {
                        row.stage = ContentStage::Failed;
                        row.error = Some(error);
                    }
                    self.content_receipt(&key);
                    self.content.active.remove(&key);
                    self.content_node(&key, "finished");
                }
            },
            WorkerEvent::Applied { key, result } => {
                let was_retry = !self.content.active.contains_key(&key);
                if let Some(row) = self.content.row_mut(&key) {
                    match result {
                        Ok(snapshot) => {
                            row.stage = ContentStage::Completed;
                            row.error = None;
                            let peer = row.peer.clone();
                            if snapshot.revision >= self.observed_revision {
                                self.observed_revision = snapshot.revision;
                                self.view.current(snapshot.clone(), Some(peer));
                                self.preview_content(&snapshot);
                            }
                        }
                        Err(_) => {
                            row.stage = ContentStage::Saved;
                            row.error = Some(TransferError::Clipboard);
                        }
                    }
                    if !was_retry {
                        self.content_receipt(&key);
                        self.content.active.remove(&key);
                        self.content_node(&key, "finished");
                    }
                }
            }
            WorkerEvent::Preview { revision, result } => {
                if self.view.snapshot.current.revision == revision
                    && let Ok((bytes, width, height)) = result
                {
                    use base64::Engine;
                    self.view.snapshot.current.preview = Some(format!(
                        "data:image/png;base64,{}",
                        base64::engine::general_purpose::STANDARD.encode(bytes)
                    ));
                    self.view.snapshot.current.image_width = Some(width);
                    self.view.snapshot.current.image_height = Some(height);
                }
            }
        }
        self.publish_snapshot();
    }
    pub(super) fn preview_content(&mut self, snapshot: &Snapshot) {
        if let Some(task) = self.preview_task.take() {
            task.abort();
        }
        if let ReadState::Ready(Payload::Image(image)) = &snapshot.content {
            let image = image.clone();
            let events = self.content.events.clone();
            let revision = snapshot.revision;
            let limit = self.content.image_limit.clone();
            self.preview_task = Some(tokio::spawn(async move {
                let Ok(permit) = limit.acquire_owned().await else {
                    return;
                };
                let result = tokio::task::spawn_blocking(move || {
                    let _permit = permit;
                    crate::preview::thumbnail(&image)
                })
                .await
                .unwrap_or(Err(nooboard_clipboard::Error::Stopped));
                let _ = events.send(WorkerEvent::Preview { revision, result }).await;
            }));
        }
    }
    fn apply_content(&self, key: String, content: Payload) {
        let clipboard = self.clipboard.clone();
        let events = self.content.events.clone();
        let lock = self.content.apply_lock.clone();
        tokio::spawn(async move {
            let _lock = lock.lock().await;
            let result = clipboard.write_content(content).await;
            let _ = events.send(WorkerEvent::Applied { key, result }).await;
        });
    }
    pub(super) fn copy_content(&mut self, key: &str) -> Result<()> {
        let row = self.content.row_mut(key).ok_or(Error::NotFound)?;
        if !row.incoming || row.saved_paths.is_empty() || row.stage.pending() {
            return Err(Error::Ineligible);
        }
        let kind = row.kind;
        let paths = row.saved_paths.clone();
        row.stage = ContentStage::Applying;
        let key = key.to_owned();
        let clipboard = self.clipboard.clone();
        let events = self.content.events.clone();
        let lock = self.content.apply_lock.clone();
        let limit = self.content.image_limit.clone();
        tokio::spawn(async move {
            let _lock = lock.lock().await;
            let result = async {
                let permit = limit
                    .acquire_owned()
                    .await
                    .map_err(|_| nooboard_clipboard::Error::Unavailable)?;
                let content = tokio::task::spawn_blocking(move || {
                    let _permit = permit;
                    saved_content(kind, paths)
                })
                .await
                .map_err(|_| nooboard_clipboard::Error::Stopped)??;
                clipboard.write_content(content).await
            }
            .await;
            let _ = events.send(WorkerEvent::Applied { key, result }).await;
        });
        self.publish_snapshot();
        Ok(())
    }
}
fn saved_content(kind: ContentKind, paths: Vec<PathBuf>) -> nooboard_clipboard::Result<Payload> {
    if kind == ContentKind::Image {
        use std::io::Read;
        let file =
            std::fs::File::open(&paths[0]).map_err(|_| nooboard_clipboard::Error::Unavailable)?;
        let mut bytes = Vec::new();
        file.take(nooboard_clipboard::MAX_IMAGE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| nooboard_clipboard::Error::Unavailable)?;
        let image = ImageData::new(ImageEncoding::Png, bytes)?;
        image.validate()?;
        Ok(Payload::Image(image))
    } else if paths.iter().all(|p| p.is_file()) {
        Ok(Payload::Files(paths))
    } else {
        Err(nooboard_clipboard::Error::Unavailable)
    }
}
