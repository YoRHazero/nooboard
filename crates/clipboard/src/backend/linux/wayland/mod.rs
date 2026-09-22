use crate::backend::PreparedWrite;
mod events;
mod objects;
mod transfer;
use super::{formats, io};
use crate::{
    Error, Options, Origin, ReadState, Result, SkipReason,
    backend::{Backend, Observation, Operation, Progress, Wake},
    formats::Candidate,
};
use events::{Payload, State};
use objects::{Device, Manager};
use std::collections::VecDeque;
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
    time::Duration,
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
    limits: crate::Limits,
    operation: Option<Pending>,
}
impl Clipboard {
    pub fn open(options: &Options) -> Result<Self> {
        let connection = Connection::connect_to_env().map_err(|_| Error::Unavailable)?;
        let (globals, mut queue) = registry_queue_init::<State>(&connection)
            .map_err(|e| Error::backend("Wayland clipboard", e))?;
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
        queue
            .roundtrip(&mut state)
            .map_err(|e| Error::backend("Wayland clipboard", e))?;
        if state.finished {
            return Err(Error::UnsupportedSession("Wayland data control was denied"));
        }
        Ok(Self {
            connection,
            queue,
            state,
            manager,
            device,
            limits: options.limits.clone(),
            operation: None,
        })
    }
    pub fn fd(&self) -> i32 {
        self.connection.as_fd().as_raw_fd()
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
}

struct Reader {
    stream: UnixStream,
    candidate: Candidate<usize>,
    bytes: Vec<u8>,
    limit: usize,
}
struct Reading {
    revision: u64,
    types: Vec<String>,
    remaining: VecDeque<Candidate<usize>>,
    reader: Reader,
}
enum Pending {
    Reading(Reading),
    Writing(crate::Payload),
    Complete(Result<Observation>),
}
impl Clipboard {
    fn snapshot(&self, content: ReadState, origin: Origin) -> Observation {
        Observation {
            revision: self.revision(),
            content,
            origin,
        }
    }
    fn begin_read(&mut self) -> Result<Pending> {
        let Some(offer) = self
            .state
            .selection
            .as_ref()
            .and_then(|id| self.state.offers.get(id))
        else {
            return Ok(Pending::Complete(Ok(
                self.snapshot(ReadState::Empty, Origin::External)
            )));
        };
        if offer.overflow {
            return Ok(Pending::Complete(Ok(self.snapshot(
                ReadState::Skipped(SkipReason::TooLarge),
                Origin::External,
            ))));
        }
        let plan = formats::plan(&offer.types);
        if self.own_selection() && plan.skipped.is_none() {
            let payload = self
                .state
                .payload
                .as_ref()
                .ok_or(Error::Changed)?
                .formats
                .content
                .clone();
            return Ok(Pending::Complete(Ok(
                self.snapshot(ReadState::Ready(payload), Origin::Application)
            )));
        }
        if plan.candidates.is_empty() {
            return Ok(Pending::Complete(Ok(
                self.snapshot(plan.fallback(), Origin::External)
            )));
        }
        let types = offer.types.clone();
        let mut remaining: VecDeque<_> = plan.candidates.into();
        let candidate = remaining.pop_front().expect("nonempty plan");
        let reader = self.reader(&types, candidate)?;
        Ok(Pending::Reading(Reading {
            revision: self.revision(),
            types,
            remaining,
            reader,
        }))
    }
    fn reader(&self, types: &[String], candidate: Candidate<usize>) -> Result<Reader> {
        let offer = self
            .state
            .selection
            .as_ref()
            .and_then(|id| self.state.offers.get(id))
            .ok_or(Error::Changed)?;
        let (stream, writer) =
            UnixStream::pair().map_err(|e| Error::backend("create Wayland data pipe", e))?;
        stream
            .set_nonblocking(true)
            .map_err(|e| Error::backend("configure Wayland data pipe", e))?;
        offer
            .proxy
            .receive(&types[candidate.format], writer.as_fd());
        self.connection
            .flush()
            .map_err(|e| Error::backend("flush Wayland request", e))?;
        drop(writer);
        Ok(Reader {
            stream,
            limit: formats::limit(&candidate, &self.limits),
            candidate,
            bytes: Vec::new(),
        })
    }
    fn begin_write(&mut self, prepared: PreparedWrite) -> Result<Pending> {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let marker = format!(
            "application/x-nooboard-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        );
        let content = prepared.payload;
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
        self.connection
            .flush()
            .map_err(|e| Error::backend("publish Wayland selection", e))?;
        Ok(Pending::Writing(content))
    }
    fn step(&mut self, operation: Pending) -> Result<Progress> {
        match operation {
            Pending::Complete(outcome) => Ok(Progress::Complete(outcome)),
            Pending::Writing(content) => {
                if self.own_selection() {
                    return Ok(Progress::Complete(Ok(
                        self.snapshot(ReadState::Ready(content), Origin::Application)
                    )));
                }
                if self.state.payload.is_none() {
                    return Err(Error::Changed);
                }
                self.operation = Some(Pending::Writing(content));
                Ok(Progress::Pending)
            }
            Pending::Reading(mut reading) => {
                if self.revision() != reading.revision {
                    return Err(Error::Changed);
                }
                let mut buffer = [0; 65536];
                for _ in 0..16 {
                    match reading.reader.stream.read(&mut buffer) {
                        Ok(0) => {
                            self.pump()?;
                            if self.revision() != reading.revision {
                                return Err(Error::Changed);
                            }
                            let reader = reading.reader;
                            let result = formats::decode(
                                reader.candidate.kind,
                                &reading.types[reader.candidate.format],
                                reader.bytes,
                            )?;
                            if let Some(content) = result {
                                return Ok(Progress::Complete(Ok(
                                    self.snapshot(content, Origin::External)
                                )));
                            }
                            let Some(candidate) = reading.remaining.pop_front() else {
                                return Ok(Progress::Complete(Ok(self.snapshot(
                                    ReadState::Skipped(SkipReason::Unsupported),
                                    Origin::External,
                                ))));
                            };
                            reading.reader = self.reader(&reading.types, candidate)?;
                            break;
                        }
                        Ok(n) => {
                            if reading.reader.bytes.len().saturating_add(n) > reading.reader.limit {
                                return Ok(Progress::Complete(Ok(self.snapshot(
                                    ReadState::Skipped(SkipReason::TooLarge),
                                    Origin::External,
                                ))));
                            }
                            reading.reader.bytes.extend_from_slice(&buffer[..n]);
                        }
                        Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                        Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                        Err(e) => return Err(Error::backend("read Wayland clipboard data", e)),
                    }
                }
                self.operation = Some(Pending::Reading(reading));
                Ok(Progress::Pending)
            }
        }
    }
}
impl Backend for Clipboard {
    fn revision(&self) -> u64 {
        self.state.revision
    }
    fn begin(&mut self, operation: Operation) -> Result<()> {
        debug_assert!(self.operation.is_none());
        self.pump()?;
        self.operation = Some(match operation {
            Operation::Read => self.begin_read()?,
            Operation::Write(payload) => self.begin_write(payload)?,
        });
        Ok(())
    }
    fn poll(&mut self) -> Result<Progress> {
        self.pump()?;
        let Some(operation) = self.operation.take() else {
            return Ok(Progress::Idle);
        };
        Ok(match self.step(operation) {
            Ok(progress) => progress,
            Err(error) => Progress::Complete(Err(error)),
        })
    }
    fn cancel(&mut self) {
        self.operation = None;
    }
    fn wait(&mut self, wake: &Wake, timeout: Duration) -> Result<()> {
        let mut fds = vec![self.fd(), wake.fd()];
        if let Some(Pending::Reading(reading)) = &self.operation {
            fds.push(reading.reader.stream.as_raw_fd());
        }
        // Service outgoing writers even when the compositor connection itself is idle.
        io::poll(&fds, Some(timeout.min(Duration::from_millis(20))))?;
        wake.drain();
        Ok(())
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
