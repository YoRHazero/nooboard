//! Accept-loop policy shared by the sync and pairing listeners. No feature state lives here.
//!
//! `accept()` errors describe one pending connection or the process's resources,
//! never the listening socket itself, so they must not end a listener's owner.
use std::{io, net::SocketAddr, time::Duration};
use tokio::{
    net::{TcpListener, TcpStream},
    time::Instant,
};

/// Long enough that descriptor or memory exhaustion cannot spin the runtime.
const RESOURCE_PAUSE: Duration = Duration::from_secs(1);

pub(crate) struct Listener {
    inner: TcpListener,
    resume_at: Option<Instant>,
}
impl Listener {
    pub fn new(inner: TcpListener) -> Self {
        Self {
            inner,
            resume_at: None,
        }
    }
    /// Waits for the next connection and never fails. Cancel-safe: a pending
    /// connection stays queued, and a pause keeps its deadline across calls, so
    /// other `select!` branches (including stop) are never blocked by it.
    pub async fn accept(&mut self) -> (TcpStream, SocketAddr) {
        loop {
            if let Some(resume_at) = self.resume_at {
                tokio::time::sleep_until(resume_at).await;
                self.resume_at = None;
            }
            match self.inner.accept().await {
                Ok(accepted) => return accepted,
                // Each such error consumes one queued connection, so retrying cannot spin.
                Err(error) if peer_aborted(&error) => {}
                Err(_) => self.resume_at = Some(Instant::now() + RESOURCE_PAUSE),
            }
        }
    }
}
/// Only errors about a single connection retry immediately. Anything else,
/// including unknown kinds, pauses instead of risking a busy loop.
fn peer_aborted(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::ConnectionAborted
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionRefused
            | io::ErrorKind::Interrupted
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_single_connection_failures_retry_without_a_pause() {
        for kind in [
            io::ErrorKind::ConnectionAborted,
            io::ErrorKind::ConnectionReset,
            io::ErrorKind::ConnectionRefused,
            io::ErrorKind::Interrupted,
        ] {
            assert!(peer_aborted(&io::Error::from(kind)), "{kind:?}");
        }
        #[cfg(unix)]
        for code in [libc::EMFILE, libc::ENFILE, libc::ENOBUFS, libc::ENOMEM] {
            assert!(!peer_aborted(&io::Error::from_raw_os_error(code)), "{code}");
        }
        assert!(!peer_aborted(&io::Error::from(io::ErrorKind::WouldBlock)));
        assert!(!peer_aborted(&io::Error::other("unknown")));
    }

    #[tokio::test(start_paused = true)]
    async fn a_pause_survives_cancellation_without_blocking_or_restarting() {
        let mut listener = Listener::new(TcpListener::bind("127.0.0.1:0").await.unwrap());
        let address = listener.inner.local_addr().unwrap();
        let started = Instant::now();
        listener.resume_at = Some(started + RESOURCE_PAUSE);
        // A competing select! branch wins while the listener is paused.
        tokio::select! {
            _ = listener.accept() => panic!("accepted during a pause"),
            _ = tokio::time::sleep(RESOURCE_PAUSE / 2) => {}
        }
        assert_eq!(listener.resume_at, Some(started + RESOURCE_PAUSE));
        let _client = TcpStream::connect(address).await.unwrap();
        listener.accept().await;
        assert!(Instant::now() >= started + RESOURCE_PAUSE);
        assert!(Instant::now() < started + RESOURCE_PAUSE * 2);
        assert_eq!(listener.resume_at, None);
    }
}
