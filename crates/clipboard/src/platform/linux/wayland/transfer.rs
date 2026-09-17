use std::{
    fs::File,
    io::{ErrorKind, Write},
    os::fd::{AsRawFd, OwnedFd},
    sync::Arc,
    time::{Duration, Instant},
};

pub(super) struct Writer {
    file: File,
    bytes: Arc<Vec<u8>>,
    offset: usize,
    deadline: Instant,
}
impl Writer {
    pub fn new(fd: OwnedFd, bytes: Arc<Vec<u8>>) -> Option<Self> {
        // SAFETY: fd is live and owned here. F_GETFL/F_SETFL do not consume it.
        let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) };
        if flags < 0 {
            return None;
        }
        // SAFETY: fd remains live; this updates only its file status flags.
        if unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return None;
        }
        Some(Self {
            file: File::from(fd),
            bytes,
            offset: 0,
            deadline: Instant::now() + Duration::from_secs(5),
        })
    }
    /// A bounded pass keeps slow readers from blocking clipboard ownership or shutdown.
    pub fn pending(&mut self) -> bool {
        if Instant::now() >= self.deadline {
            return false;
        }
        for _ in 0..16 {
            if self.offset == self.bytes.len() {
                return false;
            }
            let end = (self.offset + 65536).min(self.bytes.len());
            match self.file.write(&self.bytes[self.offset..end]) {
                Ok(0) => return false,
                Ok(n) => {
                    self.offset += n;
                    self.deadline = Instant::now() + Duration::from_secs(5);
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => return true,
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(_) => return false,
            }
        }
        self.offset < self.bytes.len()
    }
}
