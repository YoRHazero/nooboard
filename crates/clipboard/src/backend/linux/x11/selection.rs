use super::*;
pub(super) enum Data {
    Bytes {
        format: u8,
        type_: Atom,
        bytes: Vec<u8>,
    },
    TooLarge,
    Refused,
}
struct RequestWindow {
    connection: Arc<RustConnection>,
    window: Window,
}
impl Drop for RequestWindow {
    fn drop(&mut self) {
        let _ = self.connection.destroy_window(self.window);
        let _ = self.connection.flush();
    }
}
enum Stage {
    Notify,
    Incremental { bytes: Vec<u8>, type_: Option<Atom> },
}
pub(super) struct Request {
    window: RequestWindow,
    target: Atom,
    limit: usize,
    stage: Stage,
}
impl Clipboard {
    /// A fresh request window prevents late replies from cancelled operations being reused.
    pub(super) fn request(&mut self, target: Atom, limit: usize) -> Result<Request> {
        let window = self
            .connection
            .generate_id()
            .map_err(|e| Error::backend("create X11 request ID", e))?;
        self.connection
            .create_window(
                COPY_DEPTH_FROM_PARENT,
                window,
                self.window,
                0,
                0,
                1,
                1,
                0,
                WindowClass::INPUT_OUTPUT,
                0,
                &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
            )
            .map_err(|e| Error::backend("create X11 request window", e))?
            .check()
            .map_err(|e| Error::backend("create X11 request window", e))?;
        let window = RequestWindow {
            connection: self.connection.clone(),
            window,
        };
        self.connection
            .convert_selection(
                window.window,
                self.atoms.CLIPBOARD,
                target,
                self.atoms._NOOBOARD_SELECTION,
                CURRENT_TIME,
            )
            .map_err(|e| Error::backend("request X11 selection", e))?;
        self.connection
            .flush()
            .map_err(|e| Error::backend("flush X11 selection request", e))?;
        Ok(Request {
            window,
            target,
            limit,
            stage: Stage::Notify,
        })
    }
    /// A bounded pass; the runtime owns cancellation and the overall deadline.
    pub(super) fn poll_request(&mut self, request: &mut Request) -> Result<Option<Data>> {
        let window = request.window.window;
        let property = self.atoms._NOOBOARD_SELECTION;
        if matches!(request.stage, Stage::Notify) {
            let event = self.take_event(|e| matches!(e, Event::SelectionNotify(n) if n.requestor == window && n.target == request.target));
            let Some(Event::SelectionNotify(event)) = event else {
                return Ok(None);
            };
            if event.property == NONE {
                return Ok(Some(Data::Refused));
            }
            if event.property != property {
                return Err(Error::InvalidData);
            }
            let initial = self.property(window, property, request.limit)?;
            if initial.type_ != self.atoms.INCR {
                return Ok(Some(
                    if initial.bytes_after > 0 || initial.value.len() > request.limit {
                        Data::TooLarge
                    } else {
                        Data::Bytes {
                            format: initial.format,
                            type_: initial.type_,
                            bytes: initial.value,
                        }
                    },
                ));
            }
            let hint = initial
                .value32()
                .and_then(|mut v| v.next())
                .ok_or(Error::InvalidData)?;
            if hint as usize > request.limit {
                return Ok(Some(Data::TooLarge));
            }
            self.events
                .retain(|e| !matches!(e, Event::PropertyNotify(p) if p.window == window));
            self.delete_request_property(window, property)?;
            request.stage = Stage::Incremental {
                bytes: Vec::new(),
                type_: None,
            };
        }
        let Stage::Incremental { bytes, type_ } = &mut request.stage else {
            unreachable!()
        };
        for _ in 0..16 {
            if self.take_event(|e| matches!(e, Event::PropertyNotify(p) if p.window == window && p.atom == property && p.state == Property::NEW_VALUE)).is_none() { return Ok(None); }
            let chunk =
                self.property(window, property, request.limit.saturating_sub(bytes.len()))?;
            if chunk.bytes_after > 0
                || bytes.len().saturating_add(chunk.value.len()) > request.limit
            {
                return Ok(Some(Data::TooLarge));
            }
            if chunk.format != 8 || type_.is_some_and(|previous| previous != chunk.type_) {
                return Err(Error::InvalidData);
            }
            *type_ = Some(chunk.type_);
            self.delete_request_property(window, property)?;
            if chunk.value.is_empty() {
                return Ok(Some(Data::Bytes {
                    format: 8,
                    type_: chunk.type_,
                    bytes: std::mem::take(bytes),
                }));
            }
            bytes.extend_from_slice(&chunk.value);
        }
        Ok(None)
    }
    fn delete_request_property(&self, window: Window, property: Atom) -> Result<()> {
        self.connection
            .delete_property(window, property)
            .map_err(|e| Error::backend("acknowledge X11 clipboard chunk", e))?;
        self.connection
            .flush()
            .map_err(|e| Error::backend("flush X11 clipboard acknowledgement", e))
    }
    fn property(&self, window: Window, property: Atom, limit: usize) -> Result<GetPropertyReply> {
        self.connection
            .get_property(
                false,
                window,
                property,
                AtomEnum::ANY,
                0,
                (limit / 4 + 1).min(u32::MAX as usize) as u32,
            )
            .map_err(|e| Error::backend("read X11 clipboard property", e))?
            .reply()
            .map_err(|e| Error::backend("read X11 clipboard property", e))
    }
}
