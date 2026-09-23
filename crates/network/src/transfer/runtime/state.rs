use super::*;

impl Runtime {
    pub(super) fn publish(&mut self) {
        // Completion order is independent of peer IDs, namespaces and submission order.
        self.retired.retain(|key| {
            self.rows
                .get(key)
                .is_some_and(|row| row.status.stage.is_terminal() && !row.working)
        });
        for (key, row) in &mut self.rows {
            if !row.status.stage.is_terminal() || row.working {
                row.retired_stage = None;
                continue;
            }
            if row.retired_stage != Some(row.status.stage) {
                // A late authoritative outcome makes this result recent again.
                self.retired.retain(|old| old != key);
                self.retired.push_back(key.clone());
                row.retired_stage = Some(row.status.stage);
            }
        }
        while self.retired.len() > 64 {
            let Some(key) = self.retired.pop_front() else {
                break;
            };
            if let Some(row) = self.rows.remove(&key)
                && let Some(id) = row.inbox_id
            {
                self.inbox_keys.remove(&id);
            }
        }
        self.status.send_replace(Snapshot {
            rows: self.rows.values().rev().map(|r| r.status.clone()).collect(),
            ready: self
                .routes
                .iter()
                .filter(|(_, r)| r.accepting && r.remote_epoch.is_some() && !r.outbox.is_closed())
                .map(|(id, r)| (id.clone(), r.generation))
                .collect(),
        });
    }
    pub(super) fn pending(&self) -> usize {
        self.rows
            .values()
            .filter(|r| r.working || !r.status.stage.is_terminal())
            .count()
    }
    pub(super) fn context(&self, key: &Key) -> Context {
        let row = &self.rows[key];
        Context {
            key: key.clone(),
            generation: row.generation,
            epoch: self
                .routes
                .get(&key.peer)
                .and_then(|r| r.remote_epoch)
                .unwrap_or(0),
            outbox: row.outbox.clone(),
            cancel: row.cancel.clone(),
            timeout: self.timeout,
            updates: self.updates.clone(),
        }
    }
    pub(super) fn new_row(
        &self,
        key: &Key,
        route: &Route,
        stage: TransferStage,
        bytes: u64,
        text: bool,
    ) -> Row {
        Row {
            status: TransferStatus {
                id: key.id.clone(),
                peer: key.peer.clone(),
                incoming: key.incoming,
                stage,
                completed_bytes: 0,
                total_bytes: bytes,
                error: None,
                saved_paths: vec![],
            },
            generation: route.generation,
            outbox: route.outbox.clone(),
            cancel: Arc::new(AtomicBool::new(false)),
            inbox_id: None,
            manifest: None,
            deadline: Instant::now() + self.timeout,
            wire: None,
            application: None,
            working: false,
            text,
            application_report: None,
            application_dispatched: false,
            finish_sent: false,
            late_receipt: None,
            retired_stage: None,
        }
    }
    pub(super) fn finish_row(&mut self, key: &Key, stage: TransferStage, error: Option<ErrorKind>) {
        if let Some(row) = self.rows.get_mut(key) {
            row.status.stage = stage;
            row.status.error = error;
            row.wire = None;
            row.application = None;
        }
    }
    pub(super) fn disconnected(&mut self, peer: &str, generation: u64) {
        if self
            .routes
            .get(peer)
            .is_some_and(|r| r.generation == generation)
        {
            self.routes.remove(peer);
            self.lanes.remove(peer);
        }
        for (key, row) in &mut self.rows {
            if key.peer != peer || row.generation != generation || row.status.stage.is_terminal() {
                continue;
            }
            row.cancel.store(true, Ordering::Release);
            if !row.working {
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
                row.status.error = Some(ErrorKind::Unavailable);
            }
        }
    }
    pub(super) fn update(&mut self, event: WorkerEvent) {
        match event {
            WorkerEvent::Progress {
                key,
                generation,
                stage,
                done,
                total,
            } => {
                if let Some(row) = self.rows.get_mut(&key)
                    && row.generation == generation
                    && !row.status.stage.is_terminal()
                    && row.status.stage != TransferStage::Cancelling
                {
                    row.status.stage = stage;
                    row.status.completed_bytes = done;
                    row.status.total_bytes = total;
                }
            }
            WorkerEvent::Ready {
                key,
                generation,
                content,
                reply,
            } => {
                if let Some(row) = self.rows.get_mut(&key)
                    && row.generation == generation
                    && row.working
                    && !row.status.stage.is_terminal()
                {
                    if let ReceivedContent::Files(paths) = &content {
                        row.status.saved_paths = paths.clone();
                    }
                    if row.cancel.load(Ordering::Acquire) {
                        let _ = reply.send(ApplicationOutcome::Failed);
                        return;
                    }
                    row.status.stage = TransferStage::WaitingForApplication;
                    let event = NetworkEvent::ContentReady {
                        id: row.inbox_id.unwrap(),
                        transfer_id: key.id.clone(),
                        peer: key.peer,
                        content,
                    };
                    if self.inbox.try_send(event).is_ok() {
                        row.application_dispatched = true;
                        row.application = Some(reply);
                    } else {
                        let _ = reply.send(ApplicationOutcome::Rejected);
                    }
                }
            }
        }
        self.publish();
    }
    pub(super) fn finished(&mut self, finished: Finished) {
        if let Some(row) = self.rows.get_mut(&finished.key)
            && row.generation == finished.generation
        {
            row.working = false;
            row.wire = None;
            row.application = None;
            row.status.stage = finished.stage;
            row.status.error = finished.error;
            row.status.completed_bytes = finished.done;
            row.status.total_bytes = finished.total;
            if !finished.paths.is_empty() {
                row.status.saved_paths = finished.paths;
            }
            row.finish_sent = finished.finish_sent;
            if let Some(outcome) = row.application_report {
                row.status.stage = application_stage(row, outcome);
                row.status.error = None;
            }
            if let Some((result, error)) = row.late_receipt.take() {
                resolve_receipt(row, result, error);
            }
        }
        self.publish();
    }
    pub(super) fn tick(&mut self) {
        let closed = self
            .routes
            .iter()
            .filter(|(_, r)| r.outbox.is_closed())
            .map(|(id, r)| (id.clone(), r.generation))
            .collect::<Vec<_>>();
        for (peer, generation) in closed {
            self.disconnected(&peer, generation);
        }
        let now = Instant::now();
        for (key, row) in &mut self.rows {
            if row.working || row.status.stage.is_terminal() || row.deadline > now {
                continue;
            }
            if row.status.stage == TransferStage::Preparing {
                continue;
            }
            if row.status.stage == TransferStage::WaitingForAcceptance {
                let _ = row.outbox.control(receipt(
                    &key.id,
                    TransferStage::Rejected,
                    Some(ErrorKind::Timeout),
                ));
                row.status.stage = TransferStage::Rejected;
            } else if row.status.stage == TransferStage::Queued && row.outbox.cancel(&key.id) {
                row.status.stage = TransferStage::Cancelled;
            } else {
                row.status.stage = TransferStage::Unconfirmed;
            }
            row.status.error = Some(ErrorKind::Timeout);
        }
        self.publish();
    }
}

pub(super) fn application_stage(row: &Row, outcome: ApplicationOutcome) -> TransferStage {
    match outcome {
        ApplicationOutcome::Applied => TransferStage::Applied,
        _ if !row.status.saved_paths.is_empty() => TransferStage::Saved,
        ApplicationOutcome::Rejected => TransferStage::Rejected,
        _ => TransferStage::Failed,
    }
}
pub(super) fn resolve_receipt(row: &mut Row, result: ContentResult, error: Option<WireError>) {
    if row.finish_sent
        && (row.status.stage == TransferStage::Unconfirmed
            || (row.status.stage == TransferStage::Saved && result == ContentResult::Applied))
    {
        row.status.stage = match result {
            ContentResult::Applied => TransferStage::Applied,
            ContentResult::Saved => TransferStage::Saved,
            ContentResult::Rejected => TransferStage::Rejected,
            ContentResult::Cancelled => TransferStage::Cancelled,
            ContentResult::Failed => TransferStage::Failed,
        };
        row.status.error = error.map(|e| from_wire_error(e).kind());
    }
}
