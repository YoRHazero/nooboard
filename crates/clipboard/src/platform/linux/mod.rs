mod formats;
mod io;
#[cfg(test)]
mod tests;
mod wayland;
mod x11;
use crate::{Error, Result, Snapshot, worker::Command};
use std::{
    io::{Read, Write},
    os::{fd::AsRawFd, unix::net::UnixStream},
    sync::mpsc,
    time::Duration,
};

pub(crate) struct Wake {
    reader: UnixStream,
    writer: UnixStream,
}
impl Wake {
    pub fn new() -> Result<Self> {
        let (reader, writer) = UnixStream::pair().map_err(|_| Error::Native)?;
        reader.set_nonblocking(true).map_err(|_| Error::Native)?;
        writer.set_nonblocking(true).map_err(|_| Error::Native)?;
        Ok(Self { reader, writer })
    }
    pub fn notify(&self) {
        let _ = (&self.writer).write(&[1]);
    }
    fn drain(&self) {
        let mut bytes = [0; 64];
        while matches!((&self.reader).read(&mut bytes), Ok(1..)) {}
    }
}
pub(crate) enum Native {
    X11(x11::Clipboard),
    Wayland(Box<wayland::Clipboard>),
}
impl Native {
    pub fn open(max_bytes: usize) -> Result<Self> {
        let backend = std::env::var("NOOBOARD_LINUX_BACKEND").ok();
        match backend.as_deref() {
            Some("x11") => x11::Clipboard::open(max_bytes).map(Self::X11),
            Some("wayland") => wayland::Clipboard::open(max_bytes)
                .map(Box::new)
                .map(Self::Wayland),
            Some(_) => Err(Error::InvalidInput),
            None if std::env::var_os("WAYLAND_DISPLAY").is_some() => {
                wayland::Clipboard::open(max_bytes)
                    .map(Box::new)
                    .map(Self::Wayland)
            }
            None if std::env::var_os("DISPLAY").is_some() => {
                x11::Clipboard::open(max_bytes).map(Self::X11)
            }
            None => Err(Error::UnsupportedSession(
                "no X11 or Wayland display; run in a desktop session",
            )),
        }
    }
    pub fn revision(&self) -> u64 {
        match self {
            Self::X11(c) => c.revision(),
            Self::Wayland(c) => c.revision(),
        }
    }
    fn pump(&mut self) -> Result<()> {
        match self {
            Self::X11(c) => c.pump(),
            Self::Wayland(c) => c.pump(),
        }
    }
    fn fd(&self) -> i32 {
        match self {
            Self::X11(c) => c.fd(),
            Self::Wayland(c) => c.fd(),
        }
    }
    pub fn wait(
        &mut self,
        rx: &mpsc::Receiver<Command>,
        wake: &Wake,
        _: Duration,
        retry: bool,
    ) -> Result<Option<Command>> {
        let revision = self.revision();
        self.pump()?;
        match rx.try_recv() {
            Ok(command) => return Ok(Some(command)),
            Err(mpsc::TryRecvError::Disconnected) => return Err(Error::Stopped),
            Err(_) => {}
        }
        if self.revision() != revision {
            return Ok(None);
        }
        // A maintenance deadline also releases abandoned INCR transfers / Wayland writers.
        io::poll(
            &[self.fd(), wake.reader.as_raw_fd()],
            Some(Duration::from_millis(if retry { 10 } else { 100 })),
        )?;
        wake.drain();
        self.pump()?;
        match rx.try_recv() {
            Ok(command) => Ok(Some(command)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(_) => Err(Error::Stopped),
        }
    }
    pub fn read(&mut self) -> Result<Snapshot> {
        match self {
            Self::X11(c) => c.read(),
            Self::Wayland(c) => c.read(),
        }
    }
    pub fn write(&mut self, text: &str) -> Result<Snapshot> {
        match self {
            Self::X11(c) => c.write(text),
            Self::Wayland(c) => c.write(text),
        }
    }
    pub fn write_content(&mut self, content: &crate::Content) -> Result<Snapshot> {
        match self {
            Self::X11(c) => c.write_content(content),
            Self::Wayland(c) => c.write_content(content),
        }
    }
}
