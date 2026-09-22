use crate::backend::PreparedWrite;
mod selection;
mod transfer;
use super::{formats, io};
use crate::{
    Error, Options, Origin, ReadState, Result, SkipReason,
    backend::{Backend, Observation, Operation, Progress, Wake},
    formats::Candidate,
};
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
    limits: crate::Limits,
    operation: Option<Pending>,
    revision: u64,
    timestamp: Timestamp,
    owned: Option<Arc<formats::Payload>>,
    events: VecDeque<Event>,
    outgoing: Vec<Outgoing>,
}
impl Clipboard {
    pub fn open(options: &Options) -> Result<Self> {
        let (connection, screen) = x11rb::connect(None).map_err(|_| Error::Unavailable)?;
        connection
            .xfixes_query_version(5, 0)
            .map_err(|e| Error::backend("X11 clipboard", e))?
            .reply()
            .map_err(|_| {
                Error::UnsupportedSession("X11 server lacks XFixes selection notifications")
            })?;
        let window = connection
            .generate_id()
            .map_err(|e| Error::backend("X11 clipboard", e))?;
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
            .map_err(|e| Error::backend("X11 clipboard", e))?
            .check()
            .map_err(|e| Error::backend("X11 clipboard", e))?;
        let atoms = Atoms::new(&connection)
            .map_err(|e| Error::backend("X11 clipboard", e))?
            .reply()
            .map_err(|e| Error::backend("X11 clipboard", e))?;
        connection
            .xfixes_select_selection_input(
                window,
                atoms.CLIPBOARD,
                SelectionEventMask::SET_SELECTION_OWNER
                    | SelectionEventMask::SELECTION_WINDOW_DESTROY
                    | SelectionEventMask::SELECTION_CLIENT_CLOSE,
            )
            .map_err(|e| Error::backend("X11 clipboard", e))?
            .check()
            .map_err(|e| Error::backend("X11 clipboard", e))?;
        connection
            .flush()
            .map_err(|e| Error::backend("X11 clipboard", e))?;
        Ok(Self {
            connection: Arc::new(connection),
            window,
            atoms,
            limits: options.limits.clone(),
            operation: None,
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
    fn owner(&self) -> Result<Window> {
        Ok(self
            .connection
            .get_selection_owner(self.atoms.CLIPBOARD)
            .map_err(|e| Error::backend("X11 clipboard", e))?
            .reply()
            .map_err(|_| Error::Unavailable)?
            .owner)
    }
}

struct Reading {
    revision: u64,
    owner: Window,
    request: selection::Request,
    stage: ReadStage,
}
enum ReadStage {
    Targets,
    Content {
        types: Vec<String>,
        targets: Vec<u32>,
        current: Candidate<usize>,
        remaining: VecDeque<Candidate<usize>>,
    },
}
enum Pending {
    Reading(Reading),
    Writing(Arc<formats::Payload>),
    Complete(Result<Observation>),
}
impl Clipboard {
    fn snapshot(&self, content: ReadState, origin: Origin) -> Observation {
        Observation {
            revision: self.revision,
            content,
            origin,
        }
    }
    fn begin_read(&mut self) -> Result<Pending> {
        let owner = self.owner()?;
        if owner == NONE {
            return Ok(Pending::Complete(Ok(
                self.snapshot(ReadState::Empty, Origin::External)
            )));
        }
        if owner == self.window {
            let content = self.owned.as_ref().ok_or(Error::Changed)?.content.clone();
            return Ok(Pending::Complete(Ok(
                self.snapshot(ReadState::Ready(content), Origin::Application)
            )));
        }
        Ok(Pending::Reading(Reading {
            revision: self.revision,
            owner,
            request: self.request(self.atoms.TARGETS, 1024)?,
            stage: ReadStage::Targets,
        }))
    }
    fn begin_write(&mut self, prepared: PreparedWrite) -> Result<Pending> {
        let payload = Arc::new(formats::Payload::new(prepared.payload)?);
        self.events.retain(|e| !matches!(e, Event::PropertyNotify(p) if p.window == self.window && p.atom == self.atoms._NOOBOARD_TIME));
        self.connection
            .change_property8(
                PropMode::REPLACE,
                self.window,
                self.atoms._NOOBOARD_TIME,
                AtomEnum::STRING,
                &[1],
            )
            .map_err(|e| Error::backend("request X11 timestamp", e))?;
        self.connection
            .flush()
            .map_err(|e| Error::backend("flush X11 timestamp request", e))?;
        Ok(Pending::Writing(payload))
    }
    fn complete_read(&mut self, reading: &Reading, content: ReadState) -> Result<Progress> {
        self.pump()?;
        if self.revision != reading.revision || self.owner()? != reading.owner {
            return Err(Error::Changed);
        }
        Ok(Progress::Complete(Ok(
            self.snapshot(content, Origin::External)
        )))
    }
    fn step(&mut self, operation: Pending) -> Result<Progress> {
        match operation {
            Pending::Complete(outcome) => Ok(Progress::Complete(outcome)),
            Pending::Writing(payload) => {
                let window = self.window;
                let property = self.atoms._NOOBOARD_TIME;
                let event = self.take_event(|e| matches!(e, Event::PropertyNotify(p) if p.window == window && p.atom == property && p.state == Property::NEW_VALUE));
                let Some(Event::PropertyNotify(event)) = event else {
                    self.operation = Some(Pending::Writing(payload));
                    return Ok(Progress::Pending);
                };
                self.timestamp = event.time;
                self.connection
                    .set_selection_owner(self.window, self.atoms.CLIPBOARD, self.timestamp)
                    .map_err(|e| Error::backend("set X11 selection owner", e))?
                    .check()
                    .map_err(|e| Error::backend("set X11 selection owner", e))?;
                self.connection
                    .flush()
                    .map_err(|e| Error::backend("flush X11 ownership", e))?;
                if self.owner()? != self.window {
                    return Err(Error::Changed);
                }
                let content = payload.content.clone();
                self.owned = Some(payload);
                self.revision = self.revision.wrapping_add(1);
                Ok(Progress::Complete(Ok(
                    self.snapshot(ReadState::Ready(content), Origin::Application)
                )))
            }
            Pending::Reading(mut reading) => {
                if self.revision != reading.revision {
                    return Err(Error::Changed);
                }
                let Some(data) = self.poll_request(&mut reading.request)? else {
                    self.operation = Some(Pending::Reading(reading));
                    return Ok(Progress::Pending);
                };
                match &mut reading.stage {
                    ReadStage::Targets => {
                        let targets: Vec<u32> = match data {
                            selection::Data::Bytes {
                                format: 32,
                                type_,
                                bytes,
                            } if type_ == AtomEnum::ATOM.into() => bytes
                                .chunks_exact(4)
                                .map(|b| u32::from_ne_bytes(b.try_into().expect("four-byte atom")))
                                .collect(),
                            _ => {
                                return self.complete_read(
                                    &reading,
                                    ReadState::Skipped(SkipReason::Unsupported),
                                );
                            }
                        };
                        let mut types = Vec::with_capacity(targets.len());
                        for atom in &targets {
                            let name = self
                                .connection
                                .get_atom_name(*atom)
                                .map_err(|e| Error::backend("query X11 format", e))?
                                .reply()
                                .map_err(|e| Error::backend("query X11 format", e))?
                                .name;
                            types.push(String::from_utf8_lossy(&name).into_owned());
                        }
                        let plan = formats::plan(&types);
                        if plan.candidates.is_empty() {
                            return self.complete_read(&reading, plan.fallback());
                        }
                        let mut remaining: VecDeque<_> = plan.candidates.into();
                        let current = remaining.pop_front().expect("nonempty plan");
                        reading.request = self.request(
                            targets[current.format],
                            formats::limit(&current, &self.limits),
                        )?;
                        reading.stage = ReadStage::Content {
                            types,
                            targets,
                            current,
                            remaining,
                        };
                    }
                    ReadStage::Content {
                        types,
                        targets,
                        current,
                        remaining,
                    } => {
                        let content = match data {
                            selection::Data::Bytes {
                                format: 8, bytes, ..
                            } => formats::decode(current.kind, &types[current.format], bytes)?,
                            selection::Data::TooLarge => {
                                Some(ReadState::Skipped(SkipReason::TooLarge))
                            }
                            selection::Data::Refused => return Err(Error::Unavailable),
                            _ => Some(ReadState::Skipped(SkipReason::Unsupported)),
                        };
                        if let Some(content) = content {
                            return self.complete_read(&reading, content);
                        }
                        let Some(next) = remaining.pop_front() else {
                            return self.complete_read(
                                &reading,
                                ReadState::Skipped(SkipReason::Unsupported),
                            );
                        };
                        *current = next;
                        reading.request = self.request(
                            targets[current.format],
                            formats::limit(current, &self.limits),
                        )?;
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
        self.revision
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
        io::poll(
            &[self.fd(), wake.fd()],
            Some(timeout.min(Duration::from_millis(20))),
        )?;
        wake.drain();
        Ok(())
    }
}
impl Drop for Clipboard {
    fn drop(&mut self) {
        let _ = self.connection.destroy_window(self.window);
        let _ = self.connection.flush();
    }
}
