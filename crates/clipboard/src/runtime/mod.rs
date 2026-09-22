pub(crate) mod request;
mod state;
#[cfg(test)]
mod tests;

use crate::{
    Error, Options, Origin, ReadState, Result, ServiceStatus, SkipReason, Snapshot,
    backend::{Backend, Observation, Operation, PreparedWrite, Progress, Wake},
    formats,
};
use request::{Command, Request};
use state::State;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, oneshot, watch};

pub(crate) struct Channels {
    pub requests: mpsc::Receiver<Request>,
    pub snapshots: watch::Sender<Option<Snapshot>>,
    pub status: watch::Sender<ServiceStatus>,
}
enum Phase {
    Native,
    Preparing(oneshot::Receiver<Result<PreparedWrite>>),
    Retry(Instant),
}
struct Active {
    reply: Option<oneshot::Sender<Result<Snapshot>>>,
    write: bool,
    deadline: Instant,
    phase: Phase,
}
struct Runtime {
    backend: Box<dyn Backend>,
    options: Options,
    channels: Channels,
    wake: Arc<Wake>,
    stop: Arc<AtomicBool>,
    executor: tokio::runtime::Handle,
    state: State,
    active: Option<Active>,
    next_observation: Instant,
    failed_revision: Option<u64>,
    requests_since_observation: usize,
}
pub(crate) fn run(
    backend: Box<dyn Backend>,
    options: Options,
    channels: Channels,
    wake: Arc<Wake>,
    stop: Arc<AtomicBool>,
    executor: tokio::runtime::Handle,
) -> Result<()> {
    let mut runtime = Runtime {
        backend,
        options,
        channels,
        wake,
        stop,
        executor,
        state: State::default(),
        active: None,
        next_observation: Instant::now(),
        failed_revision: None,
        requests_since_observation: 0,
    };
    let outcome = runtime.run();
    runtime.backend.cancel();
    let error = outcome.as_ref().err().cloned().unwrap_or(Error::Stopped);
    if let Some(active) = runtime.active.take()
        && let Some(reply) = active.reply
    {
        let _ = reply.send(Err(error.clone()));
    }
    runtime.channels.requests.close();
    while let Ok(request) = runtime.channels.requests.try_recv() {
        let _ = request.reply.send(Err(error.clone()));
    }
    outcome
}
impl Runtime {
    fn run(&mut self) -> Result<()> {
        loop {
            if self.stop.load(Ordering::Acquire) {
                return Ok(());
            }
            if let Some(active) = &self.active {
                // A submitted native write is allowed to finish, so its attribution is
                // retained even when the caller no longer waits for its receipt.
                if !active.write && active.reply.as_ref().is_some_and(|r| r.is_closed()) {
                    self.backend.cancel();
                    self.active = None;
                } else if Instant::now() >= active.deadline
                    && !matches!(active.phase, Phase::Preparing(_))
                {
                    self.backend.cancel();
                    self.finish(Err(Error::Timeout))?;
                }
            }
            // Always pump the backend, even during CPU preparation or with no requests.
            if let Progress::Complete(outcome) = self.backend.poll()? {
                self.finish(outcome)?;
            }
            if self.advance()? {
                continue;
            }
            if self.active.is_none() {
                let revision = self.backend.revision();
                let observe =
                    self.needs_observation(revision) && Instant::now() >= self.next_observation;
                if observe && self.requests_since_observation >= 8 {
                    self.requests_since_observation = 0;
                    self.read(None)?;
                    continue;
                }
                // At most one queue entry per pass: native events cannot be starved.
                match self.channels.requests.try_recv() {
                    Ok(request) => {
                        self.requests_since_observation =
                            self.requests_since_observation.saturating_add(1);
                        if !request.reply.is_closed() {
                            self.start(request)?;
                        }
                        continue;
                    }
                    Err(mpsc::error::TryRecvError::Disconnected) => return Ok(()),
                    Err(mpsc::error::TryRecvError::Empty) => {
                        if observe {
                            self.requests_since_observation = 0;
                            self.read(None)?;
                        }
                    }
                }
                if self.active.is_some() {
                    continue;
                }
            }
            let now = Instant::now();
            let mut timeout = self.options.poll_interval;
            if let Some(active) = &self.active {
                timeout = timeout.min(active.deadline.saturating_duration_since(now));
                if let Phase::Retry(at) = active.phase {
                    timeout = timeout.min(at.saturating_duration_since(now));
                }
                if matches!(active.phase, Phase::Preparing(_)) {
                    timeout = self.options.poll_interval;
                }
            } else if self.needs_observation(self.backend.revision()) {
                timeout = timeout.min(self.next_observation.saturating_duration_since(now));
            }
            self.backend.wait(&self.wake, timeout)?;
        }
    }
    fn needs_observation(&self, revision: u64) -> bool {
        self.failed_revision != Some(revision)
            && (self.state.native_revision != Some(revision)
                || matches!(&*self.channels.status.borrow(), ServiceStatus::Unavailable(error) if error.retryable()))
    }
    fn read(&mut self, reply: Option<oneshot::Sender<Result<Snapshot>>>) -> Result<()> {
        self.active = Some(Active {
            reply,
            write: false,
            deadline: Instant::now() + self.options.operation_timeout,
            phase: Phase::Native,
        });
        let outcome = self.backend.begin(Operation::Read);
        if let Err(error) = outcome {
            self.finish(Err(error))?;
        }
        Ok(())
    }
    fn start(&mut self, request: Request) -> Result<()> {
        match request.command {
            Command::Read => self.read(Some(request.reply)),
            Command::Write(payload) => {
                if let Err(error) = formats::validate(&payload, &self.options.limits) {
                    let _ = request.reply.send(Err(error));
                    return Ok(());
                }
                let (send, receive) = oneshot::channel();
                let wake = self.wake.clone();
                let limits = self.options.limits.clone();
                // Only one preparation runs at a time. It owns bytes, never OS handles.
                self.executor.spawn_blocking(move || {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        crate::backend::prepare(payload, &limits)
                    }))
                    .unwrap_or_else(|_| {
                        Err(Error::backend("prepare clipboard data", "encoder panicked"))
                    });
                    let _ = send.send(result);
                    wake.notify();
                });
                self.active = Some(Active {
                    reply: Some(request.reply),
                    write: true,
                    deadline: Instant::now() + self.options.operation_timeout,
                    phase: Phase::Preparing(receive),
                });
                Ok(())
            }
        }
    }
    fn advance(&mut self) -> Result<bool> {
        let Some(active) = &mut self.active else {
            return Ok(false);
        };
        let operation = match &mut active.phase {
            Phase::Preparing(receive) => match receive.try_recv() {
                Ok(outcome) => {
                    if active.reply.as_ref().is_some_and(|r| r.is_closed()) {
                        self.active = None;
                        return Ok(false);
                    }
                    match outcome {
                        Ok(prepared) => {
                            active.deadline = Instant::now() + self.options.operation_timeout;
                            Operation::Write(prepared)
                        }
                        Err(error) => {
                            self.finish(Err(error))?;
                            return Ok(false);
                        }
                    }
                }
                Err(oneshot::error::TryRecvError::Empty) => return Ok(false),
                Err(_) => {
                    self.finish(Err(Error::Stopped))?;
                    return Ok(false);
                }
            },
            Phase::Retry(at) if Instant::now() >= *at => Operation::Read,
            _ => return Ok(false),
        };
        active.phase = Phase::Native;
        if let Err(error) = self.backend.begin(operation) {
            self.finish(Err(error))?;
        }
        Ok(true)
    }
    fn finish(&mut self, outcome: Result<Observation>) -> Result<()> {
        let Some(mut active) = self.active.take() else {
            return Err(Error::backend(
                "driver",
                "completion without an active operation",
            ));
        };
        // Invalid input or failed encoding does not make the native service unhealthy.
        if matches!(active.phase, Phase::Preparing(_))
            && let Err(error) = outcome
        {
            if let Some(reply) = active.reply {
                let _ = reply.send(Err(error));
            }
            return Ok(());
        }
        let outcome = match outcome {
            Err(Error::InvalidData | Error::InvalidInput) if !active.write => Ok(Observation {
                revision: self.backend.revision(),
                content: ReadState::Skipped(SkipReason::InvalidData),
                origin: Origin::External,
            }),
            outcome => outcome,
        };
        if let Err(error) = &outcome
            && !active.write
            && error.retryable()
            && Instant::now() < active.deadline
        {
            self.channels.status.send_if_modified(|s| {
                let next = ServiceStatus::Unavailable(error.clone());
                if *s == next {
                    false
                } else {
                    *s = next;
                    true
                }
            });
            active.phase = Phase::Retry(Instant::now() + self.options.retry_interval);
            self.active = Some(active);
            return Ok(());
        }
        let result = match outcome {
            Ok(mut observed) => {
                formats::bound_observation(&mut observed.content, &self.options.limits);
                let snapshot = self.state.observe(observed, active.write)?;
                self.channels.snapshots.send_if_modified(|old| {
                    if old.as_ref() == Some(&snapshot) {
                        false
                    } else {
                        *old = Some(snapshot.clone());
                        true
                    }
                });
                self.channels.status.send_if_modified(|s| {
                    if *s == ServiceStatus::Ready {
                        false
                    } else {
                        *s = ServiceStatus::Ready;
                        true
                    }
                });
                self.failed_revision = None;
                self.requests_since_observation = 0;
                Ok(snapshot)
            }
            Err(error) => {
                if !active.write {
                    self.next_observation = Instant::now()
                        + Duration::from_millis(250).max(self.options.retry_interval);
                    if !error.retryable() {
                        self.failed_revision = Some(self.backend.revision());
                    }
                }
                self.channels
                    .status
                    .send_replace(ServiceStatus::Unavailable(error.clone()));
                Err(error)
            }
        };
        if let Some(reply) = active.reply {
            let _ = reply.send(result);
        }
        Ok(())
    }
}
