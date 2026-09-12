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
impl Clipboard {
    /// A fresh request window prevents a late reply from an earlier owner being reused.
    pub(super) fn request(&mut self, target: Atom, max_bytes: usize) -> Result<Data> {
        let window = self.connection.generate_id().map_err(|_| Error::Native)?;
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
            .map_err(|_| Error::Native)?
            .check()
            .map_err(|_| Error::Native)?;
        let _window = RequestWindow {
            connection: self.connection.clone(),
            window,
        };
        let property = self.atoms._NOOBOARD_SELECTION;
        self.connection
            .convert_selection(window, self.atoms.CLIPBOARD, target, property, CURRENT_TIME)
            .map_err(|_| Error::Native)?;
        self.connection.flush().map_err(|_| Error::Native)?;
        let deadline = Instant::now() + Duration::from_secs(2);
        let event = self.await_event(deadline, |e|
            matches!(e, Event::SelectionNotify(n) if n.requestor == window && n.target == target))?;
        let Event::SelectionNotify(event) = event else {
            unreachable!()
        };
        if event.property == NONE {
            return Ok(Data::Refused);
        }
        if event.property != property {
            return Err(Error::Native);
        }
        let initial = self.property(window, property, max_bytes)?;
        if initial.type_ != self.atoms.INCR {
            return Ok(
                if initial.bytes_after > 0 || initial.value.len() > max_bytes {
                    Data::TooLarge
                } else {
                    Data::Bytes {
                        format: initial.format,
                        type_: initial.type_,
                        bytes: initial.value,
                    }
                },
            );
        }
        let hint = initial
            .value32()
            .and_then(|mut values| values.next())
            .ok_or(Error::Native)?;
        if hint as usize > max_bytes {
            return Ok(Data::TooLarge);
        }
        // The initial property notification precedes SelectionNotify; discard it before INCR.
        self.events
            .retain(|e| !matches!(e, Event::PropertyNotify(p) if p.window == window));
        self.connection
            .delete_property(window, property)
            .map_err(|_| Error::Native)?;
        self.connection.flush().map_err(|_| Error::Native)?;
        let mut output = Vec::new();
        let mut type_ = None;
        loop {
            self.await_event(deadline, |e| {
                matches!(e, Event::PropertyNotify(p)
                if p.window == window && p.atom == property && p.state == Property::NEW_VALUE)
            })?;
            let chunk = self.property(window, property, max_bytes.saturating_sub(output.len()))?;
            if chunk.bytes_after > 0 || output.len() + chunk.value.len() > max_bytes {
                return Ok(Data::TooLarge);
            }
            if chunk.format != 8 || type_.is_some_and(|previous| previous != chunk.type_) {
                return Err(Error::Native);
            }
            type_ = Some(chunk.type_);
            self.connection
                .delete_property(window, property)
                .map_err(|_| Error::Native)?;
            self.connection.flush().map_err(|_| Error::Native)?;
            if chunk.value.is_empty() {
                return Ok(Data::Bytes {
                    format: 8,
                    type_: chunk.type_,
                    bytes: output,
                });
            }
            output.extend_from_slice(&chunk.value);
        }
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
            .map_err(|_| Error::Native)?
            .reply()
            .map_err(|_| Error::Unavailable)
    }
}
