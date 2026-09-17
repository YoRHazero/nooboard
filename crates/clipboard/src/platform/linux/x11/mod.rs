mod selection;
mod transfer;
use super::{formats, io};
use crate::{Content, Error, Origin, Result, Snapshot};
use std::{
    collections::VecDeque,
    os::fd::AsRawFd,
    sync::Arc,
    time::{Duration, Instant},
};
use transfer::Outgoing;
use x11rb::{
    COPY_DEPTH_FROM_PARENT, CURRENT_TIME, NONE,
    connection::Connection,
    protocol::{
        Event,
        xfixes::{ConnectionExt as _, SelectionEventMask},
        xproto::*,
    },
    rust_connection::RustConnection,
    wrapper::ConnectionExt as _,
};

x11rb::atom_manager! {
    pub(super) Atoms: AtomsCookie {
        CLIPBOARD, TARGETS, TIMESTAMP, UTF8_STRING, INCR,
        _NOOBOARD_SELECTION, _NOOBOARD_TIME,
    }
}
pub(crate) struct Clipboard {
    connection: Arc<RustConnection>,
    window: Window,
    atoms: Atoms,
    max_bytes: usize,
    revision: u64,
    timestamp: Timestamp,
    owned: Option<Arc<formats::Payload>>,
    events: VecDeque<Event>,
    outgoing: Vec<Outgoing>,
}
impl Clipboard {
    pub fn open(max_bytes: usize) -> Result<Self> {
        let (connection, screen) = x11rb::connect(None).map_err(|_| Error::Unavailable)?;
        connection
            .xfixes_query_version(5, 0)
            .map_err(|_| Error::Native)?
            .reply()
            .map_err(|_| {
                Error::UnsupportedSession("X11 server lacks XFixes selection notifications")
            })?;
        let window = connection.generate_id().map_err(|_| Error::Native)?;
        connection
            .create_window(
                COPY_DEPTH_FROM_PARENT,
                window,
                connection.setup().roots[screen].root,
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
        let atoms = Atoms::new(&connection)
            .map_err(|_| Error::Native)?
            .reply()
            .map_err(|_| Error::Native)?;
        connection
            .xfixes_select_selection_input(
                window,
                atoms.CLIPBOARD,
                SelectionEventMask::SET_SELECTION_OWNER
                    | SelectionEventMask::SELECTION_WINDOW_DESTROY
                    | SelectionEventMask::SELECTION_CLIENT_CLOSE,
            )
            .map_err(|_| Error::Native)?
            .check()
            .map_err(|_| Error::Native)?;
        connection.flush().map_err(|_| Error::Native)?;
        Ok(Self {
            connection: Arc::new(connection),
            window,
            atoms,
            max_bytes,
            revision: 1,
            timestamp: CURRENT_TIME,
            owned: None,
            events: VecDeque::new(),
            outgoing: Vec::new(),
        })
    }
    pub fn fd(&self) -> i32 {
        self.connection.stream().as_raw_fd()
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn pump(&mut self) -> Result<()> {
        self.outgoing
            .retain(|transfer| transfer.deadline > Instant::now());
        for _ in 0..256 {
            let Some(event) = self
                .connection
                .poll_for_event()
                .map_err(|_| Error::Unavailable)?
            else {
                break;
            };
            match event {
                Event::XfixesSelectionNotify(event) if event.selection == self.atoms.CLIPBOARD => {
                    if event.owner != self.window && self.owner()? != self.window {
                        self.owned = None;
                        self.revision = self.revision.wrapping_add(1);
                    }
                }
                Event::SelectionClear(event) if event.selection == self.atoms.CLIPBOARD => {
                    if self.owner()? != self.window {
                        self.owned = None;
                    }
                }
                Event::SelectionRequest(event) => self.serve(event)?,
                Event::PropertyNotify(event)
                    if event.state == Property::DELETE && self.advance(event)? => {}
                event @ (Event::SelectionNotify(_) | Event::PropertyNotify(_)) => {
                    if self.events.len() == 128 {
                        self.events.pop_front();
                    }
                    self.events.push_back(event);
                }
                _ => {}
            }
        }
        self.connection.flush().map_err(|_| Error::Unavailable)
    }
    fn take_event(&mut self, accept: impl Fn(&Event) -> bool) -> Option<Event> {
        self.events
            .iter()
            .position(accept)
            .and_then(|index| self.events.remove(index))
    }
    fn await_event(&mut self, deadline: Instant, accept: impl Fn(&Event) -> bool) -> Result<Event> {
        loop {
            self.pump()?;
            if let Some(event) = self.take_event(&accept) {
                return Ok(event);
            }
            if Instant::now() >= deadline {
                return Err(Error::Unavailable);
            }
            io::poll(&[self.fd()], Some(Duration::from_millis(10)))?;
        }
    }
    fn owner(&self) -> Result<Window> {
        Ok(self
            .connection
            .get_selection_owner(self.atoms.CLIPBOARD)
            .map_err(|_| Error::Native)?
            .reply()
            .map_err(|_| Error::Unavailable)?
            .owner)
    }
    pub fn read(&mut self) -> Result<Snapshot> {
        self.pump()?;
        let revision = self.revision;
        let owner = self.owner()?;
        let (content, origin) = if owner == NONE {
            (Content::Empty, Origin::External)
        } else if owner == self.window {
            let payload = self.owned.as_ref().ok_or(Error::Changed)?;
            (payload.content.clone(), Origin::Application)
        } else {
            (self.read_external()?, Origin::External)
        };
        self.pump()?;
        if self.revision != revision || self.owner()? != owner {
            return Err(Error::Changed);
        }
        Ok(Snapshot {
            revision,
            content,
            origin,
        })
    }
    fn read_external(&mut self) -> Result<Content> {
        let targets = self.request(self.atoms.TARGETS, 1024)?;
        let target_ids: Vec<u32> = match targets {
            selection::Data::Bytes {
                format: 32,
                type_,
                bytes,
            } if type_ == AtomEnum::ATOM.into() => bytes
                .chunks_exact(4)
                .map(|b| u32::from_ne_bytes(b.try_into().expect("four-byte atom")))
                .collect(),
            selection::Data::TooLarge => return Ok(Content::Unsupported),
            _ => return Ok(Content::Unsupported),
        };
        let mut names = Vec::with_capacity(target_ids.len());
        for atom in &target_ids {
            let name = self
                .connection
                .get_atom_name(*atom)
                .map_err(|_| Error::Native)?
                .reply()
                .map_err(|_| Error::Native)?
                .name;
            names.push(String::from_utf8_lossy(&name).into_owned());
        }
        if formats::excluded(&names) == Some(Content::Sensitive) {
            return Ok(Content::Sensitive);
        }
        if let Some((index, mime)) = formats::FILE_TYPES
            .iter()
            .find_map(|mime| names.iter().position(|n| n == mime).map(|i| (i, *mime)))
        {
            match self.request(target_ids[index], 4 * 1024 * 1024)? {
                selection::Data::Bytes {
                    bytes, format: 8, ..
                } => {
                    if let Some(content) = formats::files(mime, &bytes) {
                        return Ok(content);
                    }
                }
                _ => return Ok(Content::Unsupported),
            }
        }
        if let Some((index, mime)) = formats::IMAGE_TYPES
            .iter()
            .find_map(|mime| names.iter().position(|n| n == mime).map(|i| (i, *mime)))
        {
            return match self.request(target_ids[index], crate::MAX_IMAGE_BYTES)? {
                selection::Data::Bytes {
                    bytes, format: 8, ..
                } => formats::image(mime, bytes),
                selection::Data::TooLarge => Ok(Content::TooLarge),
                _ => Ok(Content::Unsupported),
            };
        }
        if names.iter().any(|n| n.starts_with("image/")) {
            return Ok(Content::Unsupported);
        }
        let target = formats::UTF8_TYPES.iter().find_map(|preferred| {
            names
                .iter()
                .position(|name| name == preferred)
                .map(|i| target_ids[i])
        });
        let (target, latin1) = match target {
            Some(target) => (target, false),
            None if target_ids.contains(&AtomEnum::STRING.into()) => {
                (AtomEnum::STRING.into(), true)
            }
            _ => return Ok(Content::Unsupported),
        };
        match self.request(target, self.max_bytes)? {
            selection::Data::TooLarge => Ok(Content::TooLarge),
            selection::Data::Bytes {
                format: 8, bytes, ..
            } => {
                let text = if latin1 {
                    bytes.into_iter().map(char::from).collect()
                } else {
                    match String::from_utf8(bytes) {
                        Ok(text) => text,
                        Err(_) => return Ok(Content::Unsupported),
                    }
                };
                if text.contains('\0') {
                    Ok(Content::Unsupported)
                } else if text.len() > self.max_bytes {
                    Ok(Content::TooLarge)
                } else {
                    Ok(Content::Text(text))
                }
            }
            _ => Err(Error::Unavailable),
        }
    }
    pub fn write(&mut self, text: &str) -> Result<Snapshot> {
        if text.len() > self.max_bytes || text.contains('\0') {
            return Err(Error::InvalidInput);
        }
        self.write_content(&Content::Text(text.into()))
    }
    pub fn write_content(&mut self, content: &Content) -> Result<Snapshot> {
        if let Content::Text(text) = content
            && (text.len() > self.max_bytes || text.contains('\0'))
        {
            return Err(Error::InvalidInput);
        }
        let payload = Arc::new(formats::Payload::new(content.clone())?);
        self.pump()?;
        self.connection
            .change_property8(
                PropMode::REPLACE,
                self.window,
                self.atoms._NOOBOARD_TIME,
                AtomEnum::STRING,
                &[1],
            )
            .map_err(|_| Error::Native)?;
        self.connection.flush().map_err(|_| Error::Native)?;
        let window = self.window;
        let property = self.atoms._NOOBOARD_TIME;
        let event = self.await_event(Instant::now() + Duration::from_secs(2), |e|
            matches!(e, Event::PropertyNotify(p) if p.window == window && p.atom == property && p.state == Property::NEW_VALUE))?;
        let Event::PropertyNotify(event) = event else {
            unreachable!()
        };
        self.timestamp = event.time;
        self.connection
            .set_selection_owner(self.window, self.atoms.CLIPBOARD, self.timestamp)
            .map_err(|_| Error::Native)?
            .check()
            .map_err(|_| Error::Native)?;
        self.connection.flush().map_err(|_| Error::Native)?;
        if self.owner()? != self.window {
            return Err(Error::Changed);
        }
        self.owned = Some(payload);
        self.revision = self.revision.wrapping_add(1);
        Ok(Snapshot {
            revision: self.revision,
            content: content.clone(),
            origin: Origin::Application,
        })
    }
}
impl Drop for Clipboard {
    fn drop(&mut self) {
        let _ = self.connection.destroy_window(self.window);
        let _ = self.connection.flush();
    }
}
