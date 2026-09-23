use super::*;

impl Runtime {
    pub(super) fn request(
        &mut self,
        request: Request,
        jobs: &mut JoinSet<Completion>,
    ) -> Result<()> {
        match request {
            Request::Send { input, reply } => {
                if !reply.is_closed() {
                    let result = self.send(input, jobs);
                    if let Err(Ok(id)) = reply.send(result) {
                        let _ = self.cancel(&id);
                    }
                }
            }
            Request::Cancel { id, reply } => {
                if !reply.is_closed() {
                    let _ = reply.send(self.cancel(&id));
                }
            }
            Request::CancelIncoming { peer, id, reply } => {
                if !reply.is_closed() {
                    let _ = reply.send(self.cancel_incoming(peer, id));
                }
            }
            Request::CancelDelivery { peer, id, reply } => {
                if !reply.is_closed() {
                    let _ = reply.send(self.cancel_targets(&id, Some(&peer)));
                }
            }
            Request::Decide {
                id,
                decision,
                reply,
            } => {
                if !reply.is_closed() {
                    let _ = reply.send(self.decide(id, decision, jobs));
                }
            }
            Request::Complete { id, outcome, reply } => {
                if !reply.is_closed() {
                    let _ = reply.send(self.complete(id, outcome));
                }
            }
            Request::Accepting { accepting, reply } => {
                if !reply.is_closed() {
                    self.accepting = accepting;
                    for route in self.routes.values_mut() {
                        self.epoch = self.epoch.checked_add(1).ok_or(Failure::Internal)?;
                        route.local_epoch = self.epoch;
                        if route
                            .outbox
                            .control(Message::State {
                                epoch: route.local_epoch,
                                accepting,
                            })
                            .is_err()
                        {
                            route.outbox.close();
                        }
                    }
                    let _ = reply.send(Ok(()));
                }
            }
        }
        self.publish();
        Ok(())
    }
    pub(super) fn link(&mut self, event: LinkEvent) -> Result<()> {
        match event {
            LinkEvent::Connected {
                peer,
                generation,
                outbox,
            } => {
                // A session can be replaced before its Connected event reaches this runtime.
                if outbox.is_closed()
                    || self
                        .routes
                        .get(&peer)
                        .is_some_and(|r| r.generation >= generation)
                {
                    outbox.close();
                    return Ok(());
                }
                if let Some(old) = self.routes.get(&peer) {
                    self.disconnected(&peer, old.generation);
                }
                self.epoch = self.epoch.checked_add(1).ok_or(Failure::Internal)?;
                if outbox
                    .control(Message::State {
                        epoch: self.epoch,
                        accepting: self.accepting,
                    })
                    .is_err()
                {
                    outbox.close();
                    return Ok(());
                }
                self.routes.insert(
                    peer,
                    Route {
                        generation,
                        outbox,
                        local_epoch: self.epoch,
                        remote_epoch: None,
                        accepting: false,
                        texts: HashMap::new(),
                        offers: HashSet::new(),
                    },
                );
            }
            LinkEvent::Offline { peer, generation } => self.disconnected(&peer, generation),
            LinkEvent::Message {
                peer,
                generation,
                message,
            } => {
                if self
                    .routes
                    .get(&peer)
                    .is_some_and(|r| r.generation == generation)
                {
                    self.message(peer, message)?;
                }
            }
            LinkEvent::Started {
                peer,
                generation,
                id,
            } => {
                if let Some(row) = self.rows.get_mut(&Key {
                    peer,
                    id,
                    incoming: false,
                }) && row.generation == generation
                    && row.status.stage == TransferStage::Queued
                {
                    row.status.stage = TransferStage::Sending;
                    row.deadline = Instant::now() + self.timeout;
                }
            }
            LinkEvent::Written {
                peer,
                generation,
                id,
            } => {
                if let Some(row) = self.rows.get_mut(&Key {
                    peer,
                    id,
                    incoming: false,
                }) && row.generation == generation
                    && row.status.stage == TransferStage::Sending
                {
                    row.status.stage = TransferStage::AwaitingReceipt;
                    row.status.completed_bytes = row.status.total_bytes;
                }
            }
        }
        self.publish();
        Ok(())
    }
}
