use super::*;
const CHUNK: usize = 32 * 1024;
pub(super) struct Outgoing {
    requestor: Window,
    property: Atom,
    target: Atom,
    data: Arc<Vec<u8>>,
    offset: usize,
    pub deadline: Instant,
}
impl Clipboard {
    pub(super) fn serve(&mut self, request: SelectionRequestEvent) -> Result<()> {
        let property = if request.property == NONE {
            request.target
        } else {
            request.property
        };
        let mut accepted = false;
        if request.selection == self.atoms.CLIPBOARD
            && let Some(owned) = &self.owned
        {
            if request.target == self.atoms.TARGETS {
                let mut targets = vec![
                    self.atoms.TARGETS,
                    self.atoms.TIMESTAMP,
                    self.atoms.UTF8_STRING,
                ];
                for name in formats::UTF8_TYPES {
                    targets.push(
                        self.connection
                            .intern_atom(false, name.as_bytes())
                            .map_err(|_| Error::Native)?
                            .reply()
                            .map_err(|_| Error::Native)?
                            .atom,
                    );
                }
                if self.owned.as_ref().is_some_and(|bytes| bytes.is_ascii()) {
                    targets.push(AtomEnum::STRING.into());
                }
                accepted = self
                    .connection
                    .change_property32(
                        PropMode::REPLACE,
                        request.requestor,
                        property,
                        AtomEnum::ATOM,
                        &targets,
                    )
                    .map_err(|_| Error::Native)?
                    .check()
                    .is_ok();
            } else if request.target == self.atoms.TIMESTAMP {
                accepted = self
                    .connection
                    .change_property32(
                        PropMode::REPLACE,
                        request.requestor,
                        property,
                        AtomEnum::INTEGER,
                        &[self.timestamp],
                    )
                    .map_err(|_| Error::Native)?
                    .check()
                    .is_ok();
            } else {
                let name = self
                    .connection
                    .get_atom_name(request.target)
                    .map_err(|_| Error::Native)?
                    .reply();
                if let Ok(name) = name {
                    let valid = formats::UTF8_TYPES
                        .iter()
                        .any(|t| name.name == t.as_bytes())
                        || (request.target == AtomEnum::STRING.into()
                            && self.owned.as_ref().is_some_and(|b| b.is_ascii()));
                    if valid {
                        let data = owned.clone();
                        if data.len() <= CHUNK {
                            accepted = self
                                .connection
                                .change_property8(
                                    PropMode::REPLACE,
                                    request.requestor,
                                    property,
                                    request.target,
                                    &data,
                                )
                                .map_err(|_| Error::Native)?
                                .check()
                                .is_ok();
                        } else if self.outgoing.len() < 8 {
                            let alive = self
                                .connection
                                .change_window_attributes(
                                    request.requestor,
                                    &ChangeWindowAttributesAux::new()
                                        .event_mask(EventMask::PROPERTY_CHANGE),
                                )
                                .map_err(|_| Error::Native)?
                                .check()
                                .is_ok();
                            accepted = alive
                                && self
                                    .connection
                                    .change_property32(
                                        PropMode::REPLACE,
                                        request.requestor,
                                        property,
                                        self.atoms.INCR,
                                        &[data.len() as u32],
                                    )
                                    .map_err(|_| Error::Native)?
                                    .check()
                                    .is_ok();
                            if accepted {
                                self.outgoing.push(Outgoing {
                                    requestor: request.requestor,
                                    property,
                                    target: request.target,
                                    data,
                                    offset: 0,
                                    deadline: Instant::now() + Duration::from_secs(5),
                                });
                            }
                        }
                    }
                }
            }
        }
        let notify = SelectionNotifyEvent {
            response_type: SELECTION_NOTIFY_EVENT,
            sequence: 0,
            time: request.time,
            requestor: request.requestor,
            selection: request.selection,
            target: request.target,
            property: if accepted { property } else { NONE },
        };
        // A requester may close its window at any point; do not stop the clipboard owner.
        if let Ok(cookie) =
            self.connection
                .send_event(false, request.requestor, EventMask::NO_EVENT, notify)
        {
            let _ = cookie.check();
        }
        Ok(())
    }
    pub(super) fn advance(&mut self, event: PropertyNotifyEvent) -> Result<bool> {
        let Some(index) = self
            .outgoing
            .iter()
            .position(|t| t.requestor == event.window && t.property == event.atom)
        else {
            return Ok(false);
        };
        let transfer = &mut self.outgoing[index];
        let end = (transfer.offset + CHUNK).min(transfer.data.len());
        let finished = end == transfer.offset;
        let result = self
            .connection
            .change_property8(
                PropMode::REPLACE,
                transfer.requestor,
                transfer.property,
                transfer.target,
                &transfer.data[transfer.offset..end],
            )
            .map_err(|_| Error::Native)?
            .check();
        transfer.offset = end;
        if finished || result.is_err() {
            self.outgoing.remove(index);
        }
        Ok(true)
    }
}
