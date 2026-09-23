use super::*;

impl Runtime {
    pub(super) fn cancel_incoming(&mut self, peer: String, id: TransferId) -> Result<()> {
        let key = Key {
            peer: peer.clone(),
            id: id.clone(),
            incoming: true,
        };
        let row = self.rows.get(&key).ok_or(Failure::NotFound)?;
        if row.status.stage.is_terminal() || row.application_dispatched {
            return Err(Failure::InvalidArgument("receive cannot be cancelled"));
        }
        self.message(peer, Message::Cancel { id })
    }
    pub(super) fn decide(
        &mut self,
        id: IncomingId,
        decision: ReceiveDecision,
        jobs: &mut JoinSet<Completion>,
    ) -> Result<()> {
        let key = self.inbox_keys.get(&id).cloned().ok_or(Failure::NotFound)?;
        let row = self.rows.get_mut(&key).ok_or(Failure::NotFound)?;
        if row.status.stage != TransferStage::WaitingForAcceptance {
            return Err(Failure::InvalidArgument("receive stage"));
        }
        let manifest = row.manifest.as_ref().ok_or(Failure::Internal)?;
        let directory = match decision {
            ReceiveDecision::Reject => {
                let _ = row
                    .outbox
                    .control(receipt(&key.id, TransferStage::Rejected, None));
                self.finish_row(&key, TransferStage::Rejected, None);
                return Ok(());
            }
            ReceiveDecision::Accept { directory } => match (manifest.kind, directory) {
                (ContentKind::Files, Some(path)) if !path.as_os_str().is_empty() => path,
                (ContentKind::Image, None) => std::env::temp_dir(),
                _ => return Err(Failure::InvalidArgument("receive destination")),
            },
        };
        let manifest = row.manifest.take().unwrap();
        let (tx, rx) = mpsc::channel(8);
        row.wire = Some(tx);
        row.working = true;
        row.status.stage = TransferStage::Receiving;
        let context = self.context(&key);
        jobs.spawn(async move {
            Completion::Finished(incoming::run(context, directory, manifest, rx).await)
        });
        Ok(())
    }
    pub(super) fn complete(&mut self, id: IncomingId, outcome: ApplicationOutcome) -> Result<()> {
        let key = self.inbox_keys.get(&id).cloned().ok_or(Failure::NotFound)?;
        let row = self.rows.get_mut(&key).ok_or(Failure::NotFound)?;
        if !row.application_dispatched
            || row.application_report.is_some()
            || !matches!(
                row.status.stage,
                TransferStage::WaitingForApplication
                    | TransferStage::Unconfirmed
                    | TransferStage::Cancelling
                    | TransferStage::Saved
            )
        {
            return Err(Failure::InvalidArgument("application stage"));
        }
        if outcome == ApplicationOutcome::Saved && row.status.saved_paths.is_empty() {
            return Err(Failure::InvalidArgument("content was not saved"));
        }
        row.application_report = Some(outcome);
        if let Some(reply) = row.application.take()
            && reply.send(outcome).is_ok()
        {
            return Ok(());
        }
        // The worker may have timed out while the application was applying content.
        // Preserve this authoritative late result instead of treating cancellation as rollback.
        let stage = state::application_stage(row, outcome);
        let message = if row.text {
            if stage == TransferStage::Applied {
                Message::Applied { id: key.id.clone() }
            } else {
                Message::Rejected { id: key.id.clone() }
            }
        } else {
            receipt(&key.id, stage, None)
        };
        let _ = row.outbox.control(message);
        row.status.stage = stage;
        row.status.error = None;
        Ok(())
    }
    pub(super) fn inbound(
        &mut self,
        peer: &str,
        id: TransferId,
        epoch: u64,
        text: Option<String>,
        manifest: Option<Manifest>,
    ) -> Result<()> {
        let key = Key {
            peer: peer.into(),
            id: id.clone(),
            incoming: true,
        };
        let Some(route) = self.routes.get(peer) else {
            return Ok(());
        };
        if !self.accepting || epoch != route.local_epoch {
            let _ = route.outbox.control(if text.is_some() {
                Message::Rejected { id }
            } else {
                receipt(&id, TransferStage::Rejected, None)
            });
            return Ok(());
        }
        if let Some(row) = self.rows.get(&key) {
            if row.status.stage.is_terminal() && row.status.stage != TransferStage::Unconfirmed {
                let message = if row.text {
                    if row.status.stage == TransferStage::Applied {
                        Message::Applied { id }
                    } else {
                        Message::Rejected { id }
                    }
                } else {
                    receipt(&id, row.status.stage, row.status.error)
                };
                let _ = route.outbox.control(message);
            }
            return Ok(());
        }
        if self.pending() >= self.limit
            || self
                .rows
                .iter()
                .filter(|(k, r)| k.peer == peer && k.incoming && !r.status.stage.is_terminal())
                .count()
                >= 2
        {
            let _ = route.outbox.control(if text.is_some() {
                Message::Rejected { id }
            } else {
                receipt(&id, TransferStage::Rejected, Some(ErrorKind::Busy))
            });
            return Ok(());
        }
        let size = text
            .as_ref()
            .map(|t| t.len() as u64)
            .unwrap_or_else(|| manifest.as_ref().unwrap().bytes());
        let mut row = self.new_row(
            &key,
            route,
            if text.is_some() {
                TransferStage::WaitingForApplication
            } else {
                TransferStage::WaitingForAcceptance
            },
            size,
            text.is_some(),
        );
        let route = self.routes.get_mut(peer).unwrap();
        if text.is_some() {
            if route
                .texts
                .get(&id.session)
                .is_some_and(|sequence| *sequence >= id.sequence)
            {
                let _ = route.outbox.control(Message::Rejected { id });
                return Ok(());
            }
            if route.texts.len() >= 8 && !route.texts.contains_key(&id.session) {
                route.outbox.close();
                return Ok(());
            }
            route.texts.insert(id.session.clone(), id.sequence);
        } else {
            if route.offers.contains(&id) {
                let _ = route
                    .outbox
                    .control(receipt(&id, TransferStage::Rejected, None));
                return Ok(());
            }
            if route.offers.len() >= 1024 {
                route.outbox.close();
                return Ok(());
            }
            route.offers.insert(id.clone());
        }
        self.incoming_sequence = self
            .incoming_sequence
            .checked_add(1)
            .ok_or(Failure::Internal)?;
        let token = IncomingId(self.incoming_sequence);
        row.inbox_id = Some(token);
        row.manifest = manifest.clone();
        let event = if let Some(text) = text {
            NetworkEvent::ContentReady {
                id: token,
                transfer_id: id.clone(),
                peer: peer.into(),
                content: ReceivedContent::Text(text),
            }
        } else {
            let manifest = manifest.unwrap();
            NetworkEvent::IncomingOffer {
                id: token,
                transfer_id: id.clone(),
                peer: peer.into(),
                kind: manifest.kind,
                files: manifest.files,
            }
        };
        if self.inbox.try_send(event).is_err() {
            row.status.stage = TransferStage::Rejected;
            row.status.error = Some(ErrorKind::Busy);
            let _ = row.outbox.control(if row.text {
                Message::Rejected { id }
            } else {
                receipt(&id, TransferStage::Rejected, Some(ErrorKind::Busy))
            });
        }
        if row.text && row.status.stage == TransferStage::WaitingForApplication {
            row.application_dispatched = true;
        }
        self.inbox_keys.insert(token, key.clone());
        self.rows.insert(key, row);
        Ok(())
    }
    pub(super) fn message(&mut self, peer: String, message: Message) -> Result<()> {
        match message {
            Message::State { epoch, accepting } => {
                let route = self.routes.get_mut(&peer).unwrap();
                if epoch == 0 || route.remote_epoch.is_some_and(|old| epoch < old) {
                    route.outbox.close();
                } else {
                    route.remote_epoch = Some(epoch);
                    route.accepting = accepting;
                }
            }
            Message::Device { .. } => {}
            Message::Text {
                id,
                target_epoch,
                text,
            } => self.inbound(&peer, id, target_epoch, Some(text), None)?,
            Message::Offer {
                id,
                target_epoch,
                manifest,
            } => self.inbound(&peer, id, target_epoch, None, Some(manifest))?,
            Message::Applied { ref id } | Message::Rejected { ref id } => {
                let id = id.clone();
                let applied = matches!(message, Message::Applied { .. });
                let key = Key {
                    peer,
                    id,
                    incoming: false,
                };
                if let Some(row) = self.rows.get_mut(&key)
                    && row.text
                    && matches!(
                        row.status.stage,
                        TransferStage::Sending
                            | TransferStage::AwaitingReceipt
                            | TransferStage::Cancelling
                            | TransferStage::Unconfirmed
                    )
                {
                    row.status.stage = if applied {
                        TransferStage::Applied
                    } else {
                        TransferStage::Rejected
                    };
                    row.status.error = None;
                }
            }
            Message::Cancel { id } => {
                let key = Key {
                    peer,
                    id,
                    incoming: true,
                };
                if let Some(row) = self.rows.get_mut(&key) {
                    if row.status.stage.is_terminal()
                        && row.status.stage != TransferStage::Unconfirmed
                    {
                        let response = if row.text {
                            if row.status.stage == TransferStage::Applied {
                                Message::Applied { id: key.id.clone() }
                            } else {
                                Message::Rejected { id: key.id.clone() }
                            }
                        } else {
                            receipt(&key.id, row.status.stage, row.status.error)
                        };
                        let _ = row.outbox.control(response);
                    } else if row.working {
                        row.cancel.store(true, Ordering::Release);
                        row.status.stage = TransferStage::Cancelling;
                    } else if !row.application_dispatched {
                        let _ =
                            row.outbox
                                .control(receipt(&key.id, TransferStage::Cancelled, None));
                        row.status.stage = TransferStage::Cancelled;
                    }
                    // Content already handed to the application cannot be rolled back by cancellation.
                }
            }
            message @ (Message::Chunk { .. }
            | Message::Finish { .. }
            | Message::Accept { .. }
            | Message::ChunkAck { .. }
            | Message::Outcome { .. }) => {
                let (id, incoming) = match &message {
                    Message::Chunk { id, .. } | Message::Finish { id } => (id.clone(), true),
                    Message::Accept { id }
                    | Message::ChunkAck { id, .. }
                    | Message::Outcome { id, .. } => (id.clone(), false),
                    _ => unreachable!(),
                };
                let key = Key {
                    peer: peer.clone(),
                    id,
                    incoming,
                };
                if let Some(row) = self.rows.get_mut(&key) {
                    let late = match &message {
                        Message::Outcome { result, error, .. } => Some((*result, *error)),
                        _ => None,
                    };
                    if row.working
                        && let Some(receipt) = late
                    {
                        // Keep the receipt even if it arrives just as the worker times out.
                        row.late_receipt = Some(receipt);
                    }
                    match row.wire.as_ref().map(|sender| sender.try_send(message)) {
                        Some(Ok(())) => {}
                        Some(Err(mpsc::error::TrySendError::Full(_))) => {
                            row.cancel.store(true, Ordering::Release);
                            row.outbox.close();
                        }
                        _ => {
                            if let Some((result, error)) = late {
                                if row.working {
                                    row.late_receipt = Some((result, error));
                                } else {
                                    state::resolve_receipt(row, result, error);
                                }
                            }
                        }
                    }
                }
            }
            _ => self.routes[&peer].outbox.close(),
        }
        Ok(())
    }
}
