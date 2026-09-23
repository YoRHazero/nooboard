mod dispatch;
mod receiving;
mod sending;
mod state;

use super::{
    files::PreparedBatch,
    protocol::{ContentResult, Manifest, Message, TransferError as WireError},
    *,
};
use crate::{
    connections::{queue::Outbox, runtime::Event as LinkEvent},
    error::{ErrorKind, Failure, InternalResult as Result},
    event::NetworkEvent,
    identity::material::new_session_id,
    options::Options,
};
use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::{
    sync::{Semaphore, mpsc, oneshot, watch},
    task::JoinSet,
};

pub(crate) enum Request {
    Send {
        input: SendRequest,
        reply: oneshot::Sender<Result<TransferId>>,
    },
    Cancel {
        id: TransferId,
        reply: oneshot::Sender<Result<()>>,
    },
    CancelIncoming {
        peer: String,
        id: TransferId,
        reply: oneshot::Sender<Result<()>>,
    },
    CancelDelivery {
        peer: String,
        id: TransferId,
        reply: oneshot::Sender<Result<()>>,
    },
    Decide {
        id: IncomingId,
        decision: ReceiveDecision,
        reply: oneshot::Sender<Result<()>>,
    },
    Complete {
        id: IncomingId,
        outcome: ApplicationOutcome,
        reply: oneshot::Sender<Result<()>>,
    },
    Accepting {
        accepting: bool,
        reply: oneshot::Sender<Result<()>>,
    },
}
#[derive(Clone, Default)]
pub(crate) struct Snapshot {
    pub rows: Vec<TransferStatus>,
    pub ready: BTreeMap<String, u64>,
}
pub(crate) struct Handle {
    pub requests: mpsc::Sender<Request>,
    pub status: watch::Receiver<Snapshot>,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Key {
    pub peer: String,
    pub id: TransferId,
    pub incoming: bool,
}
struct Route {
    generation: u64,
    outbox: Outbox,
    local_epoch: u64,
    remote_epoch: Option<u64>,
    accepting: bool,
    texts: HashMap<String, u64>,
    offers: HashSet<TransferId>,
}
struct Row {
    status: TransferStatus,
    generation: u64,
    outbox: Outbox,
    cancel: Arc<AtomicBool>,
    inbox_id: Option<IncomingId>,
    manifest: Option<Manifest>,
    deadline: Instant,
    wire: Option<mpsc::Sender<Message>>,
    application: Option<oneshot::Sender<ApplicationOutcome>>,
    working: bool,
    text: bool,
    application_report: Option<ApplicationOutcome>,
    application_dispatched: bool,
    finish_sent: bool,
    late_receipt: Option<(ContentResult, Option<WireError>)>,
    retired_stage: Option<TransferStage>,
}
pub(super) struct Context {
    pub key: Key,
    pub generation: u64,
    pub epoch: u64,
    pub outbox: Outbox,
    pub cancel: Arc<AtomicBool>,
    pub timeout: Duration,
    pub updates: mpsc::Sender<WorkerEvent>,
}
impl Context {
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Acquire)
    }
    pub async fn cancelled(&self) {
        while !self.is_cancelled() {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
    pub async fn message(&self, messages: &mut mpsc::Receiver<Message>) -> Result<Message> {
        tokio::select! {
            _ = self.outbox.closed() => Err(Failure::Disconnected),
            result = tokio::time::timeout(self.timeout, messages.recv()) => {
                result.map_err(|_| Failure::Timeout)?.ok_or(Failure::Disconnected)
            },
        }
    }
    pub fn progress(&self, stage: TransferStage, done: u64, total: u64) {
        let _ = self.updates.try_send(WorkerEvent::Progress {
            key: self.key.clone(),
            generation: self.generation,
            stage,
            done,
            total,
        });
    }
    pub fn finished(
        &self,
        stage: TransferStage,
        error: Option<Failure>,
        done: u64,
        total: u64,
    ) -> Finished {
        Finished {
            key: self.key.clone(),
            generation: self.generation,
            stage,
            error: error.map(|e| e.kind()),
            done,
            total,
            paths: vec![],
            finish_sent: false,
        }
    }
}
pub(super) enum WorkerEvent {
    Progress {
        key: Key,
        generation: u64,
        stage: TransferStage,
        done: u64,
        total: u64,
    },
    Ready {
        key: Key,
        generation: u64,
        content: ReceivedContent,
        reply: oneshot::Sender<ApplicationOutcome>,
    },
}
pub(super) struct Finished {
    pub key: Key,
    pub generation: u64,
    pub stage: TransferStage,
    pub error: Option<ErrorKind>,
    pub done: u64,
    pub total: u64,
    pub paths: Vec<PathBuf>,
    pub finish_sent: bool,
}
enum Completion {
    Prepared {
        id: TransferId,
        result: Result<(ContentKind, Arc<PreparedBatch>)>,
    },
    Finished(Finished),
}
pub(crate) struct Runtime {
    requests: mpsc::Receiver<Request>,
    links: mpsc::Receiver<LinkEvent>,
    status: watch::Sender<Snapshot>,
    inbox: mpsc::Sender<NetworkEvent>,
    stop: watch::Receiver<bool>,
    updates: mpsc::Sender<WorkerEvent>,
    incoming: mpsc::Receiver<WorkerEvent>,
    routes: BTreeMap<String, Route>,
    rows: BTreeMap<Key, Row>,
    retired: VecDeque<Key>,
    inbox_keys: HashMap<IncomingId, Key>,
    preparing: BTreeMap<TransferId, Arc<AtomicBool>>,
    lanes: BTreeMap<String, Arc<Semaphore>>,
    namespace: String,
    sequence: u64,
    incoming_sequence: u64,
    epoch: u64,
    accepting: bool,
    limit: usize,
    timeout: Duration,
}
impl Runtime {
    pub fn new(
        options: &Options,
        links: mpsc::Receiver<LinkEvent>,
        inbox: mpsc::Sender<NetworkEvent>,
        stop: watch::Receiver<bool>,
    ) -> Result<(Self, Handle)> {
        let (tx, requests) = mpsc::channel(options.queue_capacity);
        let (status, rx) = watch::channel(Snapshot::default());
        let (updates, incoming) = mpsc::channel(64);
        Ok((
            Self {
                requests,
                links,
                status,
                inbox,
                stop,
                updates,
                incoming,
                routes: BTreeMap::new(),
                rows: BTreeMap::new(),
                retired: VecDeque::new(),
                inbox_keys: HashMap::new(),
                preparing: BTreeMap::new(),
                lanes: BTreeMap::new(),
                namespace: new_session_id()?,
                sequence: 0,
                incoming_sequence: 0,
                epoch: 0,
                accepting: true,
                limit: options.max_transfers,
                timeout: options.operation_timeout,
            },
            Handle {
                requests: tx,
                status: rx,
            },
        ))
    }
    pub async fn run(mut self) -> Result<()> {
        let mut jobs = JoinSet::new();
        let mut tick = tokio::time::interval(Duration::from_millis(100));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let result = async {
            loop {
                if *self.stop.borrow() {
                    break;
                }
                tokio::select! {
                    _ = self.stop.changed() => break,
                    event = self.links.recv() => match event {
                        Some(event) => self.link(event)?,
                        None => break,
                    },
                    request = self.requests.recv() => match request {
                        Some(request) => self.request(request, &mut jobs)?,
                        None => break,
                    },
                    Some(event) = self.incoming.recv() => self.update(event),
                    result = jobs.join_next(), if !jobs.is_empty() => {
                        match result.ok_or(Failure::Internal)?.map_err(|_| Failure::Internal)? {
                            Completion::Prepared { id, result } => self.prepared(id, result, &mut jobs),
                            Completion::Finished(finished) => self.finished(finished),
                        }
                    },
                    _ = tick.tick() => self.tick(),
                }
            }
            Ok(())
        }.await;
        self.requests.close();
        for cancel in self.preparing.values() {
            cancel.store(true, Ordering::Release);
        }
        for row in self.rows.values_mut() {
            row.cancel.store(true, Ordering::Release);
            row.application.take();
        }
        // Await staging/publication workers: aborting spawn_blocking would leave writes running after shutdown.
        while !jobs.is_empty() {
            tokio::select! {
                Some(event) = self.incoming.recv() => {
                    if let WorkerEvent::Ready { reply, .. } = event {
                        let _ = reply.send(ApplicationOutcome::Failed);
                    }
                },
                result = jobs.join_next() => match result {
                    Some(Ok(Completion::Finished(finished))) => self.finished(finished),
                    Some(Ok(Completion::Prepared { id, .. })) => { self.preparing.remove(&id); },
                    _ => {},
                }
            }
        }
        for row in self.rows.values_mut() {
            if !row.status.stage.is_terminal() {
                row.status.stage = if matches!(
                    row.status.stage,
                    TransferStage::Preparing
                        | TransferStage::Queued
                        | TransferStage::WaitingForAcceptance
                ) {
                    TransferStage::Cancelled
                } else {
                    TransferStage::Unconfirmed
                };
                row.status.error = Some(ErrorKind::Stopped);
            }
        }
        self.publish();
        result
    }
}

pub(super) fn receipt(id: &TransferId, stage: TransferStage, error: Option<ErrorKind>) -> Message {
    let result = match stage {
        TransferStage::Applied => ContentResult::Applied,
        TransferStage::Saved => ContentResult::Saved,
        TransferStage::Rejected => ContentResult::Rejected,
        TransferStage::Cancelled => ContentResult::Cancelled,
        _ => ContentResult::Failed,
    };
    let error = error.map(|e| match e {
        ErrorKind::Timeout => WireError::Timeout,
        ErrorKind::Cancelled => WireError::Cancelled,
        ErrorKind::TooLarge => WireError::TooLarge,
        ErrorKind::Integrity => WireError::Integrity,
        ErrorKind::Busy => WireError::Busy,
        ErrorKind::Protocol => WireError::Protocol,
        _ => WireError::Io,
    });
    Message::Outcome {
        id: id.clone(),
        result,
        error,
    }
}
pub(super) fn from_wire_error(error: WireError) -> Failure {
    match error {
        WireError::Timeout => Failure::Timeout,
        WireError::Cancelled => Failure::Cancelled,
        WireError::TooLarge => Failure::TooLarge,
        WireError::SourceChanged => Failure::SourceChanged,
        WireError::Integrity => Failure::Integrity,
        WireError::Busy => Failure::Busy,
        WireError::Protocol => Failure::Protocol,
        WireError::Offline => Failure::Disconnected,
        _ => Failure::Protocol,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn runtime() -> Runtime {
        let (_tx, rx) = mpsc::channel(1);
        let (inbox, _) = mpsc::channel(1);
        let (_, stop) = watch::channel(false);
        Runtime::new(&Options::default(), rx, inbox, stop)
            .unwrap()
            .0
    }
    #[test]
    fn a_closed_or_superseded_connection_event_never_stops_the_service() {
        let mut runtime = runtime();
        let stale = Outbox::new();
        stale.close();
        runtime
            .link(LinkEvent::Connected {
                peer: "peer".into(),
                generation: 1,
                outbox: stale,
            })
            .unwrap();
        assert!(runtime.routes.is_empty());
        let current = Outbox::new();
        runtime
            .link(LinkEvent::Connected {
                peer: "peer".into(),
                generation: 3,
                outbox: current.clone(),
            })
            .unwrap();
        let stale = Outbox::new();
        runtime
            .link(LinkEvent::Connected {
                peer: "peer".into(),
                generation: 2,
                outbox: stale.clone(),
            })
            .unwrap();
        assert!(stale.is_closed());
        assert!(!current.is_closed());
        assert_eq!(runtime.routes["peer"].generation, 3);
        runtime
            .link(LinkEvent::Offline {
                peer: "peer".into(),
                generation: 2,
            })
            .unwrap();
        assert!(!current.is_closed());
        assert_eq!(runtime.routes["peer"].generation, 3);
    }
    #[test]
    fn delayed_ready_events_cannot_reopen_finished_work() {
        let mut runtime = runtime();
        runtime
            .link(LinkEvent::Connected {
                peer: "peer".into(),
                generation: 1,
                outbox: Outbox::new(),
            })
            .unwrap();
        let key = Key {
            peer: "peer".into(),
            id: TransferId {
                session: "0123456789abcdef0123456789abcdef".into(),
                sequence: 1,
            },
            incoming: true,
        };
        let mut row = runtime.new_row(
            &key,
            &runtime.routes["peer"],
            TransferStage::Receiving,
            8,
            false,
        );
        row.working = true;
        row.inbox_id = Some(IncomingId(1));
        runtime.rows.insert(key.clone(), row);
        let finished = runtime
            .context(&key)
            .finished(TransferStage::Unconfirmed, None, 8, 8);
        runtime.finished(finished);
        let (reply, mut response) = oneshot::channel();
        runtime.update(WorkerEvent::Ready {
            key: key.clone(),
            generation: 1,
            content: ReceivedContent::Image(vec![]),
            reply,
        });
        assert_eq!(runtime.rows[&key].status.stage, TransferStage::Unconfirmed);
        assert!(!runtime.rows[&key].application_dispatched);
        assert_eq!(
            response.try_recv(),
            Err(oneshot::error::TryRecvError::Closed)
        );
    }
    #[test]
    fn retention_uses_completion_order_and_keeps_active_and_working_rows() {
        let mut runtime = runtime();
        runtime
            .link(LinkEvent::Connected {
                peer: "route".into(),
                generation: 1,
                outbox: Outbox::new(),
            })
            .unwrap();
        let key = |peer: &str, sequence| Key {
            peer: peer.into(),
            id: TransferId {
                session: "0123456789abcdef0123456789abcdef".into(),
                sequence,
            },
            incoming: true,
        };
        let insert = |runtime: &mut Runtime, key: Key, stage, working, token| {
            let mut row = runtime.new_row(&key, &runtime.routes["route"], stage, 1, false);
            row.working = working;
            row.inbox_id = Some(IncomingId(token));
            runtime.inbox_keys.insert(IncomingId(token), key.clone());
            runtime.rows.insert(key, row);
            runtime.publish();
        };
        let delayed = key("a", 1);
        insert(
            &mut runtime,
            delayed.clone(),
            TransferStage::Receiving,
            false,
            1,
        );
        let active = key("b", 1);
        insert(
            &mut runtime,
            active.clone(),
            TransferStage::Receiving,
            false,
            2,
        );
        let working = key("c", 1);
        insert(&mut runtime, working.clone(), TransferStage::Saved, true, 3);
        // IDs deliberately run backwards: key order differs from result order.
        for n in 1..=64 {
            insert(
                &mut runtime,
                key("z", 65 - n),
                TransferStage::Unconfirmed,
                false,
                100 + n,
            );
        }
        runtime.finish_row(&delayed, TransferStage::Applied, None);
        runtime.publish();
        assert!(
            runtime.rows.contains_key(&delayed),
            "newest completion was immediately evicted"
        );
        assert!(
            !runtime.rows.contains_key(&key("z", 64)),
            "oldest completion was retained"
        );
        assert!(!runtime.inbox_keys.contains_key(&IncomingId(101)));
        assert!(runtime.inbox_keys.contains_key(&IncomingId(1)));
        assert!(runtime.rows.contains_key(&active));
        assert!(runtime.rows[&working].working);
        assert_eq!(runtime.rows.len(), 66);

        // A late authoritative result becomes recent again.
        let late = key("z", 63);
        runtime.finish_row(&late, TransferStage::Applied, None);
        runtime.publish();
        insert(&mut runtime, key("a", 2), TransferStage::Saved, false, 4);
        assert_eq!(runtime.rows[&late].status.stage, TransferStage::Applied);
        assert!(!runtime.rows.contains_key(&key("z", 62)));
        assert_eq!(runtime.rows.len(), 66);
    }
}
