use super::*;

impl Runtime {
    pub(super) fn send(
        &mut self,
        input: SendRequest,
        jobs: &mut JoinSet<Completion>,
    ) -> Result<TransferId> {
        input.validate()?;
        if self.pending() + input.targets.len() > self.limit
            || (!matches!(input.content, OutgoingContent::Text(_)) && self.preparing.len() >= 4)
        {
            return Err(Failure::Busy);
        }
        self.sequence = self.sequence.checked_add(1).ok_or(Failure::Internal)?;
        let id = TransferId {
            session: self.namespace.clone(),
            sequence: self.sequence,
        };
        let text = matches!(input.content, OutgoingContent::Text(_));
        let size = match &input.content {
            OutgoingContent::Text(s) => s.len() as u64,
            OutgoingContent::Image(s) => s.len() as u64,
            _ => 0,
        };
        for peer in input.targets {
            let key = Key {
                id: id.clone(),
                peer: peer.clone(),
                incoming: false,
            };
            let fallback = Route {
                generation: 0,
                outbox: Outbox::new(),
                local_epoch: 0,
                remote_epoch: None,
                accepting: false,
                texts: HashMap::new(),
                offers: HashSet::new(),
            };
            let route = self.routes.get(&peer).unwrap_or(&fallback);
            let mut row = self.new_row(
                &key,
                route,
                if text {
                    TransferStage::Queued
                } else {
                    TransferStage::Preparing
                },
                size,
                text,
            );
            if route.outbox.is_closed() || !route.accepting || route.remote_epoch.is_none() {
                row.status.stage = TransferStage::Failed;
                row.status.error = Some(ErrorKind::Unavailable);
            } else if let (OutgoingContent::Text(value), Some(epoch)) =
                (&input.content, route.remote_epoch)
            {
                match route.outbox.text(
                    Message::Text {
                        id: id.clone(),
                        target_epoch: epoch,
                        text: value.clone(),
                    },
                    input.queue.clone(),
                ) {
                    Ok(Some(old)) => self.finish_row(
                        &Key {
                            peer: peer.clone(),
                            id: old,
                            incoming: false,
                        },
                        TransferStage::Superseded,
                        None,
                    ),
                    Ok(None) => {}
                    Err(error) => {
                        row.status.stage = TransferStage::Failed;
                        row.status.error = Some(error.kind());
                    }
                }
            }
            self.rows.insert(key, row);
        }
        if !text
            && self
                .rows
                .iter()
                .any(|(k, r)| k.id == id && !k.incoming && !r.status.stage.is_terminal())
        {
            let cancel = Arc::new(AtomicBool::new(false));
            self.preparing.insert(id.clone(), cancel.clone());
            let prepared_id = id.clone();
            jobs.spawn(async move {
                let result = tokio::task::spawn_blocking(move || {
                    files::check_cancelled(&cancel)?;
                    let (kind, batch) = match input.content {
                        OutgoingContent::Files(paths) => (
                            ContentKind::Files,
                            PreparedBatch::from_paths(&paths, &cancel, |_, _| {})?,
                        ),
                        OutgoingContent::Image(bytes) => (
                            ContentKind::Image,
                            PreparedBatch::from_bytes("image.png", &bytes)?,
                        ),
                        _ => unreachable!(),
                    };
                    files::check_cancelled(&cancel)?;
                    Ok((kind, Arc::new(batch)))
                })
                .await
                .unwrap_or(Err(Failure::Internal));
                Completion::Prepared {
                    id: prepared_id,
                    result,
                }
            });
        }
        Ok(id)
    }
    pub(super) fn prepared(
        &mut self,
        id: TransferId,
        result: Result<(ContentKind, Arc<PreparedBatch>)>,
        jobs: &mut JoinSet<Completion>,
    ) {
        self.preparing.remove(&id);
        let keys = self
            .rows
            .iter()
            .filter(|(k, r)| {
                k.id == id && !k.incoming && r.status.stage == TransferStage::Preparing
            })
            .map(|(k, _)| k.clone())
            .collect::<Vec<_>>();
        for key in keys {
            let live = self.routes.get(&key.peer).is_some_and(|r| {
                r.generation == self.rows[&key].generation
                    && r.accepting
                    && r.remote_epoch.is_some()
                    && !r.outbox.is_closed()
            });
            match &result {
                Ok((kind, batch)) if live => {
                    let (tx, rx) = mpsc::channel(8);
                    let row = self.rows.get_mut(&key).unwrap();
                    row.wire = Some(tx);
                    row.working = true;
                    row.status.total_bytes = batch.bytes();
                    row.status.stage = TransferStage::Queued;
                    let context = self.context(&key);
                    let kind = *kind;
                    let batch = batch.clone();
                    let lane = self
                        .lanes
                        .entry(key.peer.clone())
                        .or_insert_with(|| Arc::new(Semaphore::new(1)))
                        .clone();
                    jobs.spawn(async move {
                        Completion::Finished(outgoing::run(context, kind, batch, lane, rx).await)
                    });
                }
                Err(error) => self.finish_row(&key, TransferStage::Failed, Some(error.kind())),
                _ => self.finish_row(&key, TransferStage::Failed, Some(ErrorKind::Unavailable)),
            }
        }
    }
    pub(super) fn cancel(&mut self, id: &TransferId) -> Result<()> {
        self.cancel_targets(id, None)
    }
    pub(super) fn cancel_targets(&mut self, id: &TransferId, peer: Option<&str>) -> Result<()> {
        let mut found = false;
        for (key, row) in &mut self.rows {
            if key.id != *id
                || key.incoming
                || row.status.stage.is_terminal()
                || peer.is_some_and(|peer| peer != key.peer)
            {
                continue;
            }
            found = true;
            row.cancel.store(true, Ordering::Release);
            if row.text {
                if row.outbox.cancel(id) {
                    row.status.stage = TransferStage::Cancelled;
                } else {
                    row.status.stage = TransferStage::Cancelling;
                    let _ = row.outbox.control(Message::Cancel { id: id.clone() });
                }
            } else if row.working {
                row.status.stage = TransferStage::Cancelling;
            } else {
                row.status.stage = TransferStage::Cancelled;
            }
        }
        // Preparation is shared. Stop it only after every destination has retired.
        if !self
            .rows
            .iter()
            .any(|(key, row)| key.id == *id && !key.incoming && !row.status.stage.is_terminal())
            && let Some(cancel) = self.preparing.get(id)
        {
            cancel.store(true, Ordering::Release);
        }
        if found {
            Ok(())
        } else {
            Err(Failure::NotFound)
        }
    }
}
