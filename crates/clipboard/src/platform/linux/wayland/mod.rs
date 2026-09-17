mod events;
mod objects;
mod transfer;
use super::{formats, io};
use crate::{Content, Error, Origin, Result, Snapshot};
use events::{Payload, State};
use objects::{Device, Manager};
use std::{
    io::{ErrorKind, Read},
    os::{
        fd::{AsFd, AsRawFd},
        unix::net::UnixStream,
    },
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use wayland_client::{
    Connection, EventQueue, globals::registry_queue_init, protocol::wl_seat::WlSeat,
};

pub(crate) struct Clipboard {
    connection: Connection,
    queue: EventQueue<State>,
    state: State,
    manager: Manager,
    device: Device,
    max_bytes: usize,
}
impl Clipboard {
    pub fn open(max_bytes: usize) -> Result<Self> {
        let connection = Connection::connect_to_env().map_err(|_| Error::Unavailable)?;
        let (globals, mut queue) =
            registry_queue_init::<State>(&connection).map_err(|_| Error::Native)?;
        let q = queue.handle();
        let manager = if let Ok(m) = globals.bind(&q, 1..=1, ()) {
            Manager::Ext(m)
        } else if let Ok(m) = globals.bind(&q, 1..=2, ()) {
            Manager::Wlr(m)
        } else {
            return Err(Error::UnsupportedSession(
                "Wayland compositor must expose ext-data-control-v1 or wlr-data-control-v1",
            ));
        };
        let seat: WlSeat = globals
            .bind(&q, 1..=9, ())
            .map_err(|_| Error::UnsupportedSession("Wayland session has no seat"))?;
        let device = manager.device(&seat, &q);
        let mut state = State {
            revision: 1,
            ..State::default()
        };
        queue.roundtrip(&mut state).map_err(|_| Error::Native)?;
        if state.finished {
            return Err(Error::UnsupportedSession("Wayland data control was denied"));
        }
        Ok(Self {
            connection,
            queue,
            state,
            manager,
            device,
            max_bytes,
        })
    }
    pub fn fd(&self) -> i32 {
        self.connection.as_fd().as_raw_fd()
    }
    pub fn revision(&self) -> u64 {
        self.state.revision
    }
    pub fn pump(&mut self) -> Result<()> {
        self.queue
            .dispatch_pending(&mut self.state)
            .map_err(|_| Error::Stopped)?;
        for _ in 0..16 {
            self.connection.flush().map_err(|_| Error::Stopped)?;
            if !io::poll(&[self.fd()], Some(Duration::ZERO))?[0] {
                break;
            }
            if let Some(guard) = self.queue.prepare_read() {
                match guard.read() {
                    Ok(_) => {}
                    Err(wayland_client::backend::WaylandError::Io(e))
                        if e.kind() == ErrorKind::WouldBlock => {}
                    Err(_) => return Err(Error::Stopped),
                }
            }
            self.queue
                .dispatch_pending(&mut self.state)
                .map_err(|_| Error::Stopped)?;
        }
        self.state.writers.retain_mut(|w| w.pending());
        if self.state.finished {
            return Err(Error::Stopped);
        }
        Ok(())
    }
    fn own_selection(&self) -> bool {
        self.state.payload.as_ref().is_some_and(|p| {
            self.state
                .selection
                .as_ref()
                .and_then(|id| self.state.offers.get(id))
                .is_some_and(|o| o.types.contains(&p.marker))
        })
    }
    fn snapshot(&self, content: Content, origin: Origin) -> Snapshot {
        Snapshot {
            revision: self.revision(),
            content,
            origin,
        }
    }
    pub fn read(&mut self) -> Result<Snapshot> {
        self.pump()?;
        let Some(offer) = self
            .state
            .selection
            .as_ref()
            .and_then(|id| self.state.offers.get(id))
        else {
            return Ok(self.snapshot(Content::Empty, Origin::External));
        };
        if offer.overflow {
            return Ok(self.snapshot(Content::Unsupported, Origin::External));
        }
        if formats::excluded(&offer.types) == Some(Content::Sensitive) {
            return Ok(self.snapshot(Content::Sensitive, Origin::External));
        }
        if self.own_selection() {
            let content = self
                .state
                .payload
                .as_ref()
                .ok_or(Error::Changed)?
                .formats
                .content
                .clone();
            return Ok(self.snapshot(content, Origin::Application));
        }
        let types = offer.types.clone();
        if let Some(mime) = formats::FILE_TYPES
            .iter()
            .find(|m| types.iter().any(|t| t == **m))
        {
            let Some(bytes) = self.read_mime(mime, 4 * 1024 * 1024)? else {
                return Ok(self.snapshot(Content::TooLarge, Origin::External));
            };
            if let Some(content) = formats::files(mime, &bytes) {
                return Ok(self.snapshot(content, Origin::External));
            }
        }
        if let Some(mime) = formats::IMAGE_TYPES
            .iter()
            .find(|m| types.iter().any(|t| t == **m))
        {
            let content = match self.read_mime(mime, crate::MAX_IMAGE_BYTES)? {
                Some(bytes) => formats::image(mime, bytes)?,
                None => Content::TooLarge,
            };
            return Ok(self.snapshot(content, Origin::External));
        }
        if types.iter().any(|t| t.starts_with("image/")) {
            return Ok(self.snapshot(Content::Unsupported, Origin::External));
        }
        let Some(mime) = formats::UTF8_TYPES
            .iter()
            .find(|m| types.iter().any(|t| t == **m))
        else {
            return Ok(self.snapshot(Content::Unsupported, Origin::External));
        };
        let content = match self.read_mime(mime, self.max_bytes)? {
            Some(bytes) => match String::from_utf8(bytes) {
                Ok(text) if !text.contains('\0') => Content::Text(text),
                _ => Content::Unsupported,
            },
            None => Content::TooLarge,
        };
        Ok(self.snapshot(content, Origin::External))
    }
    fn read_mime(&mut self, mime: &str, limit: usize) -> Result<Option<Vec<u8>>> {
        let offer = self
            .state
            .selection
            .as_ref()
            .and_then(|id| self.state.offers.get(id))
            .ok_or(Error::Changed)?;
        let revision = self.revision();
        let (mut reader, writer) = UnixStream::pair().map_err(|_| Error::Native)?;
        reader.set_nonblocking(true).map_err(|_| Error::Native)?;
        offer.proxy.receive(mime, writer.as_fd());
        self.connection.flush().map_err(|_| Error::Stopped)?;
        drop(writer);
        let mut deadline = Instant::now() + Duration::from_secs(5);
        let mut bytes = Vec::new();
        let mut chunk = [0; 65536];
        loop {
            self.pump()?;
            if revision != self.revision() {
                return Err(Error::Changed);
            }
            match reader.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    if bytes.len().saturating_add(n) > limit {
                        return Ok(None);
                    }
                    bytes.extend_from_slice(&chunk[..n]);
                    deadline = Instant::now() + Duration::from_secs(5);
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => {
                    self.wait_until(deadline, &[self.fd(), reader.as_raw_fd()])?
                }
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(_) => return Err(Error::Unavailable),
            }
        }
        Ok(Some(bytes))
    }
    fn wait_until(&self, deadline: Instant, fds: &[i32]) -> Result<()> {
        let left = deadline
            .checked_duration_since(Instant::now())
            .ok_or(Error::Unavailable)?;
        io::poll(fds, Some(left.min(Duration::from_millis(20))))?;
        Ok(())
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
        self.pump()?;
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let marker = format!(
            "application/x-nooboard-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        );
        let payload = Arc::new(Payload {
            formats: formats::Payload::new(content.clone())?,
            marker,
        });
        let source = self.manager.source(&self.queue.handle(), payload.clone());
        for mime in payload.formats.data.keys() {
            source.offer(mime);
        }
        source.offer(&payload.marker);
        self.device.select(&source);
        if let Some(old) = self.state.source.replace(source) {
            old.destroy();
        }
        self.state.payload = Some(payload);
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            self.pump()?;
            if self.own_selection() {
                return Ok(self.snapshot(content.clone(), Origin::Application));
            }
            if self.state.payload.is_none() {
                return Err(Error::Changed);
            }
            self.wait_until(deadline, &[self.fd()])?;
        }
    }
}
impl Drop for Clipboard {
    fn drop(&mut self) {
        self.device.destroy();
        if let Some(source) = self.state.source.take() {
            source.destroy();
        }
        for (_, offer) in self.state.offers.drain() {
            offer.proxy.destroy();
        }
        let _ = self.connection.flush();
    }
}
