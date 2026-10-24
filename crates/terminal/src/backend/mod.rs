//! Protocol and operating-system drivers; no application configuration ownership.
pub(crate) mod files;
mod local;
mod process;
pub(crate) mod ssh;
mod tunnel;

use crate::output::{OutputBuffer, ReadyFilter};
use crate::*;
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::sync::{mpsc, oneshot};
pub(crate) type Reply<T> = oneshot::Sender<Result<T>>;
pub(crate) enum Input {
    Write(Vec<u8>, Reply<()>),
    Resize(TerminalSize, Reply<()>),
}
pub(crate) enum DriverSpec {
    Local {
        command: Option<String>,
    },
    Ssh {
        target: SshStartTarget,
        command: Option<String>,
    },
    Tunnel {
        target: SshStartTarget,
        forward: TunnelForwardConfig,
    },
}
pub(crate) enum DriverEvent {
    Finished {
        id: RunId,
        result: Result<()>,
    },
    FileStarted(FileOperationRunId),
    FileFinished {
        id: FileOperationRunId,
        completion: files::FileCompletion,
    },
    ShortFinished(u64),
    Started {
        id: RunId,
        connection: Option<Arc<ssh::Connection>>,
    },
    Resolved {
        id: RunId,
        address: SocketAddr,
    },
    Ready(RunId),
    Cleaning(RunId),
    Forced(RunId),
    StopFailed {
        id: RunId,
        error: TerminalError,
    },
    FileProgress {
        id: FileOperationRunId,
        progress: FileOperationProgress,
    },
}
pub(crate) struct Context {
    pub id: RunId,
    pub options: TerminalServiceOptions,
    pub host_verifier: Arc<dyn SshHostVerifier>,
    pub events: mpsc::UnboundedSender<DriverEvent>,
    pub stop: CancellationToken,
    pub force: CancellationToken,
    pub children_drained: CancellationToken,
    pub transport: Arc<Mutex<Option<Arc<ssh::TransportControl>>>>,
    pub size: Arc<Mutex<Option<TerminalSize>>>,
    pub input: mpsc::Receiver<Input>,
    pub output: Option<Arc<OutputSink>>,
}
pub(crate) struct OutputSink {
    pub buffer: Arc<OutputBuffer>,
    filter: Mutex<Option<ReadyFilter>>,
    ready_sent: std::sync::atomic::AtomicBool,
    id: RunId,
    events: mpsc::UnboundedSender<DriverEvent>,
}
impl OutputSink {
    pub fn new(
        buffer: Arc<OutputBuffer>,
        command: bool,
        id: RunId,
        events: mpsc::UnboundedSender<DriverEvent>,
    ) -> Arc<Self> {
        Arc::new(Self {
            buffer,
            filter: Mutex::new(command.then(ReadyFilter::default)),
            ready_sent: std::sync::atomic::AtomicBool::new(false),
            id,
            events,
        })
    }
    pub fn push(&self, data: &[u8]) {
        let mut filter = self.filter.lock().unwrap();
        if let Some(filter) = filter.as_mut() {
            let (bytes, ready) = filter.feed(data);
            self.buffer.push(&bytes);
            if ready
                && !self
                    .ready_sent
                    .swap(true, std::sync::atomic::Ordering::AcqRel)
            {
                let _ = self.events.send(DriverEvent::Ready(self.id));
            }
        } else {
            self.buffer.push(data);
        }
    }
    pub fn finish(&self) {
        if let Some(filter) = self.filter.lock().unwrap().as_mut() {
            self.buffer.push(&filter.finish());
        }
        self.buffer.close();
    }
}
impl Context {
    pub fn event(&self, event: DriverEvent) {
        let _ = self.events.send(event);
    }
    pub fn owner(&self) -> SshConnectionOwnerRunId {
        match self.id {
            RunId::Terminal(id) => SshConnectionOwnerRunId::Terminal(id),
            RunId::Command(id) => SshConnectionOwnerRunId::Command(id),
            RunId::Tunnel(id) => SshConnectionOwnerRunId::Tunnel(id),
            RunId::FileOperation(_) => unreachable!("file operations use their parent connection"),
        }
    }
}
pub(crate) async fn run(spec: DriverSpec, context: Context) -> Result<()> {
    let output = context.output.clone();
    let transport = context.transport.clone();
    let result = match spec {
        DriverSpec::Local { command } => local::run(command, context).await,
        DriverSpec::Ssh { target, command } => ssh::run(target, command, context).await,
        DriverSpec::Tunnel { target, forward } => tunnel::run(target, forward, context).await,
    };
    let transport = transport.lock().unwrap().clone();
    if let Some(transport) = transport {
        transport.close();
        transport.ended.cancelled().await;
    }
    if let Some(output) = output {
        output.finish();
    }
    result
}
pub(crate) fn driver_error(error: impl std::fmt::Display) -> TerminalError {
    TerminalError::new(TerminalErrorKind::ProcessFailed, error.to_string())
}
