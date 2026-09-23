use super::{
    ApplicationOutcome, ContentKind, ReceivedContent, TransferStage,
    files::{FileSpec, IncomingBatch},
    protocol::{Manifest, Message},
    runtime::{Context, Finished, WorkerEvent},
};
use crate::error::{Failure, InternalResult as Result};
use std::path::PathBuf;
use tokio::sync::{mpsc, oneshot};

pub(super) async fn run(
    context: Context,
    directory: PathBuf,
    manifest: Manifest,
    mut messages: mpsc::Receiver<Message>,
) -> Finished {
    let total = manifest.bytes();
    let result = receive(&context, directory, manifest, &mut messages).await;
    let finished = match result {
        Ok((stage, paths)) => {
            let mut finished = context.finished(stage, None, total, total);
            finished.paths = paths;
            finished
        }
        Err(error) => context.finished(
            if matches!(error, Failure::Cancelled) {
                TransferStage::Cancelled
            } else {
                TransferStage::Failed
            },
            Some(error),
            0,
            total,
        ),
    };
    // The receiver owns the final receipt, including when an application result arrives late.
    if finished.stage != TransferStage::Unconfirmed {
        let _ = context.outbox.control(super::runtime::receipt(
            &context.key.id,
            finished.stage,
            finished.error,
        ));
    }
    finished
}
async fn receive(
    context: &Context,
    directory: PathBuf,
    manifest: Manifest,
    messages: &mut mpsc::Receiver<Message>,
) -> Result<(TransferStage, Vec<PathBuf>)> {
    let specs = manifest
        .files
        .iter()
        .map(|f| FileSpec {
            name: f.name.clone(),
            bytes: f.bytes,
            sha256: f.sha256,
        })
        .collect();
    let cancel = context.cancel.clone();
    let mut transaction = tokio::task::spawn_blocking(move || {
        super::files::check_cancelled(&cancel)?;
        IncomingBatch::create(&directory, specs)
    })
    .await
    .map_err(|_| Failure::Internal)??;
    if context.is_cancelled() {
        return Err(Failure::Cancelled);
    }
    context.outbox.control(Message::Accept {
        id: context.key.id.clone(),
    })?;
    let mut done = 0;
    loop {
        let message = tokio::select! {
            message = context.message(messages) => message?,
            _ = context.cancelled() => return Err(Failure::Cancelled),
        };
        match message {
            Message::Chunk {
                file,
                offset,
                bytes,
                ..
            } => {
                let count = bytes.len() as u64;
                let cancel = context.cancel.clone();
                transaction = tokio::task::spawn_blocking(move || {
                    super::files::check_cancelled(&cancel)?;
                    transaction.write(file as usize, offset, &bytes)?;
                    Ok::<_, Failure>(transaction)
                })
                .await
                .map_err(|_| Failure::Internal)??;
                context.outbox.control(Message::ChunkAck {
                    id: context.key.id.clone(),
                    file,
                    offset: offset + count,
                })?;
                done += count;
                context.progress(TransferStage::Receiving, done, manifest.bytes());
            }
            Message::Finish { .. } => {
                let cancel = context.cancel.clone();
                let kind = manifest.kind;
                // Once publication begins it is awaited, not aborted. Cancellation may lose to commit.
                let content = tokio::task::spawn_blocking(move || {
                    super::files::check_cancelled(&cancel)?;
                    transaction.complete()?;
                    match kind {
                        ContentKind::Files => transaction.commit().map(ReceivedContent::Files),
                        ContentKind::Image => {
                            let bytes = transaction.read_staged(0, 64 * 1024 * 1024)?;
                            if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
                                return Err(Failure::Protocol);
                            }
                            Ok(ReceivedContent::Image(bytes))
                        }
                    }
                })
                .await
                .map_err(|_| Failure::Internal)??;
                let paths = match &content {
                    ReceivedContent::Files(paths) => paths.clone(),
                    _ => vec![],
                };
                let saved = !paths.is_empty();
                let (reply, response) = oneshot::channel();
                if context
                    .updates
                    .try_send(WorkerEvent::Ready {
                        key: context.key.clone(),
                        generation: context.generation,
                        content,
                        reply,
                    })
                    .is_err()
                {
                    return if saved {
                        Ok((TransferStage::Saved, paths))
                    } else {
                        Err(Failure::Busy)
                    };
                }
                let outcome = tokio::select! {
                    result = tokio::time::timeout(context.timeout, response) => result.ok().and_then(|r| r.ok()),
                    _ = context.cancelled() => None,
                    _ = context.outbox.closed() => None,
                };
                let stage = match outcome {
                    Some(ApplicationOutcome::Applied) => TransferStage::Applied,
                    _ if saved => TransferStage::Saved,
                    Some(ApplicationOutcome::Rejected) => TransferStage::Rejected,
                    Some(ApplicationOutcome::Failed) => TransferStage::Failed,
                    _ => TransferStage::Unconfirmed,
                };
                return Ok((stage, paths));
            }
            Message::Cancel { .. } => return Err(Failure::Cancelled),
            _ => return Err(Failure::Protocol),
        }
    }
}
