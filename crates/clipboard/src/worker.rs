use crate::{
    Content, Error, Origin, Result, Snapshot,
    platform::{Native, Wake},
};
use std::{
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};
use tokio::sync::{oneshot, watch};

pub(crate) enum Command {
    Read(oneshot::Sender<Result<Snapshot>>),
    Write(String, oneshot::Sender<Result<Snapshot>>),
    WriteContent(Content, oneshot::Sender<Result<Snapshot>>),
    Stop,
}
pub struct Clipboard {
    commands: mpsc::Sender<Command>,
    wake: Arc<Wake>,
    events: watch::Receiver<Result<Snapshot>>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Clipboard {
    /// Startup only records the revision; existing clipboard text is not published.
    pub fn open(interval: Duration, max_bytes: usize) -> Result<Self> {
        Self::open_with(interval, max_bytes, move || Native::open(max_bytes))
    }
    /// Opt-in diagnostics: a named private pasteboard on macOS, the current
    /// display on Linux/Windows. Use an isolated desktop on those platforms.
    #[cfg(feature = "diagnostics")]
    pub fn open_diagnostic(name: String, interval: Duration, max_bytes: usize) -> Result<Self> {
        Self::open_with(interval, max_bytes, move || {
            #[cfg(target_os = "macos")]
            {
                Native::open_named(&name, max_bytes)
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = name;
                Native::open(max_bytes)
            }
        })
    }
    fn open_with(
        interval: Duration,
        max_bytes: usize,
        open: impl FnOnce() -> Result<Native> + Send + 'static,
    ) -> Result<Self> {
        if interval.is_zero() || max_bytes == 0 {
            return Err(Error::InvalidInput);
        }
        let (commands, receiver) = mpsc::channel();
        let (events, event_receiver) = watch::channel(Ok(Snapshot {
            revision: 0,
            content: Content::Empty,
            origin: Origin::External,
        }));
        let (ready, readiness) = mpsc::sync_channel(1);
        let wake = Arc::new(Wake::new()?);
        let worker_wake = wake.clone();
        let thread = thread::Builder::new()
            .name("nooboard-clipboard".into())
            .spawn(move || {
                let mut native = match open() {
                    Ok(native) => native,
                    Err(error) => {
                        let _ = ready.send(Err(error));
                        return;
                    }
                };
                let mut revision = native.revision();
                let _ = events.send_replace(Ok(Snapshot {
                    revision,
                    content: Content::Empty,
                    origin: Origin::External,
                }));
                let _ = ready.send(Ok(()));
                let mut retry = false;
                loop {
                    let command = match native.wait(&receiver, &worker_wake, interval, retry) {
                        Ok(command) => command,
                        Err(_) => break,
                    };
                    match command {
                        Some(Command::Stop) => break,
                        Some(Command::Read(reply)) => {
                            let result = native.read();
                            if let Ok(snapshot) = &result
                                && snapshot.revision != revision
                            {
                                revision = snapshot.revision;
                                let _ = events.send_replace(Ok(snapshot.clone()));
                            }
                            let _ = reply.send(result);
                        }
                        Some(Command::Write(text, reply)) => {
                            let result = native.write(&text);
                            if let Ok(snapshot) = &result {
                                revision = snapshot.revision;
                                let _ = events.send_replace(Ok(snapshot.clone()));
                            }
                            let _ = reply.send(result);
                        }
                        Some(Command::WriteContent(content, reply)) => {
                            let result = native.write_content(&content);
                            if let Ok(snapshot) = &result {
                                revision = snapshot.revision;
                                let _ = events.send_replace(Ok(snapshot.clone()));
                            }
                            let _ = reply.send(result);
                        }
                        None => {}
                    }
                    if native.revision() != revision {
                        match native.read() {
                            Ok(snapshot) => {
                                revision = snapshot.revision;
                                let _ = events.send_replace(Ok(snapshot));
                                retry = false;
                            }
                            Err(error) => {
                                let _ = events.send_replace(Err(error));
                                retry = true;
                            }
                        }
                    }
                }
            })
            .map_err(|_| Error::Native)?;
        readiness.recv().map_err(|_| Error::Stopped)??;
        Ok(Self {
            commands,
            wake,
            events: event_receiver,
            thread: Some(thread),
        })
    }
    /// Latest snapshot channel. Rapid intermediate copies may coalesce.
    pub fn subscribe(&self) -> watch::Receiver<Result<Snapshot>> {
        self.events.clone()
    }
    pub async fn read(&self) -> Result<Snapshot> {
        let (tx, rx) = oneshot::channel();
        self.dispatch(Command::Read(tx))?;
        rx.await.map_err(|_| Error::Stopped)?
    }
    pub async fn write_text(&self, text: String) -> Result<Snapshot> {
        let (tx, rx) = oneshot::channel();
        self.dispatch(Command::Write(text, tx))?;
        rx.await.map_err(|_| Error::Stopped)?
    }
    pub async fn write_content(&self, content: Content) -> Result<Snapshot> {
        let (tx, rx) = oneshot::channel();
        self.dispatch(Command::WriteContent(content, tx))?;
        rx.await.map_err(|_| Error::Stopped)?
    }
    fn dispatch(&self, command: Command) -> Result<()> {
        self.commands.send(command).map_err(|_| Error::Stopped)?;
        self.wake.notify();
        Ok(())
    }
    pub async fn shutdown(mut self) -> Result<()> {
        let _ = self.dispatch(Command::Stop);
        if let Some(thread) = self.thread.take() {
            tokio::task::spawn_blocking(move || thread.join())
                .await
                .map_err(|_| Error::Stopped)?
                .map_err(|_| Error::Stopped)?;
        }
        Ok(())
    }
}
impl Drop for Clipboard {
    fn drop(&mut self) {
        let _ = self.dispatch(Command::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
