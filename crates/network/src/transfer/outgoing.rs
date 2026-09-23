use super::{
    ContentKind, TransferStage,
    files::PreparedBatch,
    protocol::{ContentResult, FileEntry, MAX_CHUNK_BYTES, Manifest, Message},
    runtime::{Context, Finished},
};
use crate::error::{Failure, InternalResult as Result};
use std::sync::Arc;
use tokio::{
    io::AsyncReadExt,
    sync::{Semaphore, mpsc},
};

pub(super) async fn run(
    context: Context,
    kind: ContentKind,
    data: Arc<PreparedBatch>,
    lane: Arc<Semaphore>,
    mut messages: mpsc::Receiver<Message>,
) -> Finished {
    let total = data.bytes();
    let permit = tokio::select! {
        permit = lane.acquire_owned() => permit.ok(),
        _ = context.cancelled() => None,
        _ = context.outbox.closed() => None,
    };
    let Some(_permit) = permit else {
        return context.finished(TransferStage::Cancelled, Some(Failure::Cancelled), 0, total);
    };
    if context.is_cancelled() {
        return context.finished(TransferStage::Cancelled, Some(Failure::Cancelled), 0, total);
    }
    let manifest = Manifest {
        kind,
        files: data
            .files
            .iter()
            .map(|f| FileEntry {
                name: f.name.clone(),
                bytes: f.bytes,
                sha256: f.sha256,
            })
            .collect(),
    };
    if let Err(error) = context.outbox.control(Message::Offer {
        id: context.key.id.clone(),
        target_epoch: context.epoch,
        manifest,
    }) {
        return context.finished(TransferStage::Failed, Some(error), 0, total);
    }
    let mut finish_sent = false;
    let mut done = 0;
    let result = tokio::select! {
        result = send(&context, data, &mut messages, &mut finish_sent, &mut done) => result,
        _ = context.cancelled() => Err(Failure::Cancelled),
    };
    let result = match result {
        Err(Failure::Cancelled) => {
            context.outbox.cancel(&context.key.id);
            let _ = context.outbox.control(Message::Cancel {
                id: context.key.id.clone(),
            });
            tokio::time::timeout(context.timeout, async {
                loop {
                    match context.message(&mut messages).await? {
                        Message::Outcome { result, error, .. } => {
                            return outcome(result, error, finish_sent);
                        }
                        Message::Accept { .. } | Message::ChunkAck { .. } => {}
                        _ => return Err(Failure::Protocol),
                    }
                }
            })
            .await
            .unwrap_or(Err(Failure::Timeout))
        }
        result => result,
    };
    let mut finished = match result {
        Ok((stage, error)) => context.finished(stage, error, done, total),
        Err(error) => {
            context.outbox.cancel(&context.key.id);
            let _ = context.outbox.control(Message::Cancel {
                id: context.key.id.clone(),
            });
            context.finished(TransferStage::Unconfirmed, Some(error), done, total)
        }
    };
    finished.finish_sent = finish_sent;
    finished
}
async fn send(
    context: &Context,
    data: Arc<PreparedBatch>,
    messages: &mut mpsc::Receiver<Message>,
    finish_sent: &mut bool,
    done: &mut u64,
) -> Result<(TransferStage, Option<Failure>)> {
    context.progress(TransferStage::WaitingForAcceptance, 0, data.bytes());
    match context.message(messages).await? {
        Message::Accept { .. } => {}
        Message::Outcome { result, error, .. } => return outcome(result, error, false),
        _ => return Err(Failure::Protocol),
    }
    for index in 0..data.files.len() {
        let batch = data.clone();
        let file = tokio::task::spawn_blocking(move || batch.open(index))
            .await
            .map_err(|_| Failure::Internal)??;
        let mut file = tokio::fs::File::from_std(file);
        let mut offset = 0;
        while offset < data.files[index].bytes {
            let mut bytes =
                vec![0; ((data.files[index].bytes - offset) as usize).min(MAX_CHUNK_BYTES)];
            file.read_exact(&mut bytes).await?;
            let end = offset + bytes.len() as u64;
            tokio::time::timeout(
                context.timeout,
                context.outbox.bulk(Message::Chunk {
                    id: context.key.id.clone(),
                    file: index as u16,
                    offset,
                    bytes,
                }),
            )
            .await
            .map_err(|_| Failure::Timeout)??;
            match context.message(messages).await? {
                Message::ChunkAck { file, offset, .. } if file == index as u16 && offset == end => {
                }
                Message::Outcome { result, error, .. } => return outcome(result, error, false),
                _ => return Err(Failure::Protocol),
            }
            *done += end - offset;
            offset = end;
            context.progress(TransferStage::Sending, *done, data.bytes());
        }
    }
    context.outbox.control(Message::Finish {
        id: context.key.id.clone(),
    })?;
    *finish_sent = true;
    context.progress(TransferStage::AwaitingReceipt, *done, data.bytes());
    match context.message(messages).await? {
        Message::Outcome { result, error, .. } => outcome(result, error, true),
        _ => Err(Failure::Protocol),
    }
}
fn outcome(
    result: ContentResult,
    error: Option<super::protocol::TransferError>,
    finish_sent: bool,
) -> Result<(TransferStage, Option<Failure>)> {
    if !finish_sent && matches!(result, ContentResult::Applied | ContentResult::Saved) {
        return Err(Failure::Protocol);
    }
    let failure = error.map(super::runtime::from_wire_error);
    Ok((
        match result {
            ContentResult::Applied => TransferStage::Applied,
            ContentResult::Saved => TransferStage::Saved,
            ContentResult::Rejected => TransferStage::Rejected,
            ContentResult::Cancelled => TransferStage::Cancelled,
            ContentResult::Failed => TransferStage::Failed,
        },
        failure,
    ))
}
