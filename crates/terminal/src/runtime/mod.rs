mod entry;
mod files;
mod lifecycle;
mod schedule;
#[cfg(test)]
mod tests;
use crate::*;
use crate::{
    backend::{self, DriverEvent, DriverSpec, Input, OutputSink, Reply},
    output::OutputBuffer,
    subscription::Shared,
};
use entry::*;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::{sync::mpsc, task::JoinSet};

pub(crate) enum Request {
    Terminal(
        StartTerminalRequest,
        bool,
        Reply<(TerminalRunId, Option<TerminalOutputSubscription>)>,
    ),
    Tunnel(StartTunnelRequest, Reply<TunnelRunId>),
    Command(
        StartCommandRequest,
        CancellationToken,
        bool,
        Reply<(CommandRunId, Option<TerminalOutputSubscription>)>,
    ),
    Stop(RunId, Reply<()>),
    Lifetime(CommandRunId, CommandLifetimePolicy, Reply<()>),
    Continue(DependencyRunId, Reply<()>),
    Input(TerminalIoTarget, Vec<u8>, Reply<()>),
    Resize(TerminalIoTarget, TerminalSize, Reply<()>),
    Output(TerminalIoTarget, Reply<TerminalOutputSubscription>),
    Options(TerminalServiceOptions, Reply<()>),
    Shutdown(Option<Reply<()>>),
    File(StartFileOperationRequest, Reply<FileOperationRunId>),
    StopFile(FileOperationRunId, Reply<()>),
    List(
        TerminalRunId,
        Option<RemotePath>,
        CancellationToken,
        Reply<RemoteDirectoryListing>,
    ),
    Mkdir(TerminalRunId, RemotePath, Reply<()>),
    Rename(TerminalRunId, RemotePath, RemotePath, Reply<()>),
}
pub(crate) struct Manager {
    leases: Option<Arc<dyn RunLeaseProvider>>,
    shared: Arc<Shared>,
    options: TerminalServiceOptions,
    verifier: Arc<dyn SshHostVerifier>,
    inbox: mpsc::Receiver<Request>,
    owner_stop: CancellationToken,
    tx: mpsc::UnboundedSender<DriverEvent>,
    events: mpsc::UnboundedReceiver<DriverEvent>,
    entries: HashMap<RunId, Entry>,
    dependencies: HashMap<DependencyRunId, Dependency>,
    files: HashMap<FileOperationRunId, FileEntry>,
    shorts: HashMap<u64, ShortOperation>,
    pending: Vec<PendingCommand>,
    sequence: u64,
    closing: bool,
    shutdown_waiters: Vec<Reply<()>>,
    tasks: JoinSet<()>,
    owners: HashMap<tokio::task::Id, TaskOwner>,
    timers: JoinSet<Timer>,
    notifications: Vec<TerminalEvent>,
}
impl Manager {
    pub(crate) fn new(
        shared: Arc<Shared>,
        options: TerminalServiceOptions,
        verifier: Arc<dyn SshHostVerifier>,
        inbox: mpsc::Receiver<Request>,
        owner_stop: CancellationToken,
        leases: Option<Arc<dyn RunLeaseProvider>>,
    ) -> Self {
        let (tx, events) = mpsc::unbounded_channel();
        Self {
            leases,
            shared,
            options,
            verifier,
            inbox,
            owner_stop,
            tx,
            events,
            entries: HashMap::new(),
            dependencies: HashMap::new(),
            files: HashMap::new(),
            shorts: HashMap::new(),
            pending: vec![],
            sequence: 0,
            closing: false,
            shutdown_waiters: vec![],
            tasks: JoinSet::new(),
            owners: HashMap::new(),
            timers: JoinSet::new(),
            notifications: vec![],
        }
    }
    pub(crate) async fn run(mut self) {
        let mut inbox_open = true;
        loop {
            tokio::select! {
                event = self.events.recv() => {
                    if let Some(event) = event { self.event(event); }
                }
                request = self.inbox.recv(), if inbox_open => match request {
                    Some(request) => self.request(request),
                    None => { inbox_open = false; self.begin_shutdown(None); }
                },
                _ = self.owner_stop.cancelled(), if !self.closing => self.begin_shutdown(None),
                timer = self.timers.join_next(), if !self.timers.is_empty() => {
                    if let Some(Ok(timer)) = timer { self.timer(timer); }
                }
                task = self.tasks.join_next_with_id(), if !self.tasks.is_empty() => {
                    if let Some(task) = task {
                        match task {
                            Ok((id, ())) => { self.owners.remove(&id); }
                            Err(error) => {
                                if let Some(owner) = self.owners.remove(&error.id()) {
                                    let failure = TerminalError::new(
                                        TerminalErrorKind::StopFailed,
                                        format!("resource worker failed: {error}"),
                                    );
                                    match owner {
                                        TaskOwner::Run(id) => self.stop_failed(id, failure),
                                        TaskOwner::File(id) => self.file_stop_failed(id, failure),
                                        TaskOwner::Short(id) => {
                                            if let Some(short) = self.shorts.remove(&id) {
                                                self.children_changed(RunId::Terminal(short.owner));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            self.publish();
            if self.closing
                && self.entries.is_empty()
                && self.files.is_empty()
                && self.shorts.is_empty()
            {
                self.inbox.close();
                while let Ok(request) = self.inbox.try_recv() {
                    self.request(request);
                }
                for reply in self.shutdown_waiters.drain(..) {
                    let _ = reply.send(Ok(()));
                }
                self.timers.abort_all();
                while self.timers.join_next().await.is_some() {}
                while self.tasks.join_next().await.is_some() {}
                self.shared.close();
                return;
            }
        }
    }
    fn publish(&mut self) {
        let mut entries: Vec<_> = self.entries.values().collect();
        entries.sort_by_key(|e| e.order);
        let mut snapshot = TerminalSnapshot::default();
        for entry in entries {
            match entry.kind {
                Kind::Terminal(_) => snapshot.terminal_runs.push(entry.terminal()),
                Kind::Command { .. } => snapshot.command_runs.push(entry.command()),
                Kind::Tunnel { .. } => snapshot.tunnel_runs.push(entry.tunnel()),
            }
        }
        snapshot.dependency_runs = self.dependencies.values().map(|d| d.view.clone()).collect();
        snapshot.file_operation_runs = self.files.values().map(|f| f.view.clone()).collect();
        self.shared
            .publish(snapshot, std::mem::take(&mut self.notifications));
    }
    fn new_entry(
        &mut self,
        id: RunId,
        name: String,
        target: RunTarget,
        kind: Kind,
        driver: DriverSpec,
        deps: Vec<DependencyStartSpec>,
    ) {
        let mut references = match &kind {
            Kind::Terminal(id) => id
                .map(TerminalConfigReference::Terminal)
                .into_iter()
                .collect::<Vec<_>>(),
            Kind::Command { config_id, .. } => vec![TerminalConfigReference::Command(*config_id)],
            Kind::Tunnel { config_id, .. } => vec![TerminalConfigReference::Tunnel(*config_id)],
        };
        if let RunTarget::Ssh(target) = &target {
            references.push(TerminalConfigReference::SshTarget(target.config_id));
        }
        for dependency in &deps {
            references.push(TerminalConfigReference::Command(
                dependency.command.config_id,
            ));
            if let CommandStartTarget::Ssh(target) = &dependency.command.target {
                references.push(TerminalConfigReference::SshTarget(target.config_id));
            }
        }
        references.sort();
        references.dedup();
        let config_lease = self
            .leases
            .as_ref()
            .filter(|_| !references.is_empty())
            .map(|leases| leases.acquire(id, &references));
        self.sequence += 1;
        let (input, input_rx) = mpsc::channel(self.options.input_queue_capacity);
        let output = (!matches!(id, RunId::Tunnel(_))).then(|| {
            OutputSink::new(
                OutputBuffer::new(self.options.output_buffer_bytes),
                matches!(id, RunId::Command(_)),
                id,
                self.tx.clone(),
            )
        });
        let mut entry = Entry {
            _config_lease: config_lease,
            id,
            order: self.sequence,
            name,
            target,
            kind,
            phase: if deps.is_empty() {
                Phase::Starting
            } else {
                Phase::Waiting
            },
            dependencies: vec![],
            driver: Some(driver),
            input_rx: Some(input_rx),
            input,
            output,
            size: Arc::new(Mutex::new(None)),
            stop: CancellationToken::new(),
            force: CancellationToken::new(),
            children_drained: CancellationToken::new(),
            connection: None,
            transport: Arc::new(Mutex::new(None)),
            launched: false,
            cleaning: false,
            stop_requested: false,
            ending_error: None,
            method: None,
            waiters: vec![],
            finished: None,
        };
        if !matches!(id, RunId::Terminal(_)) {
            entry.children_drained.cancel();
        }
        for spec in deps {
            let dep_id = DependencyRunId::new();
            self.dependencies.insert(
                dep_id,
                Dependency {
                    wait_order: None,
                    view: DependencyRun {
                        id: dep_id,
                        owner_run_id: entry.owner().unwrap(),
                        command_config_id: spec.command.config_id,
                        reuse_policy: spec.reuse_policy,
                        command_run_id: None,
                        gate_state: DependencyGateState::Pending,
                    },
                    spec,
                },
            );
            entry.dependencies.push(dep_id);
        }
        self.entries.insert(id, entry);
    }
    fn launch(&mut self, id: RunId) {
        let Some(entry) = self.entries.get_mut(&id) else {
            return;
        };
        if entry.launched || entry.stopping() {
            return;
        }
        let Some(spec) = entry.driver.take() else {
            return;
        };
        entry.phase = Phase::Starting;
        entry.launched = true;
        let context = backend::Context {
            id,
            options: self.options.clone(),
            host_verifier: self.verifier.clone(),
            events: self.tx.clone(),
            stop: entry.stop.clone(),
            force: entry.force.clone(),
            children_drained: entry.children_drained.clone(),
            transport: entry.transport.clone(),
            size: entry.size.clone(),
            input: entry.input_rx.take().unwrap(),
            output: entry.output.clone(),
        };
        let tx = self.tx.clone();
        let task = self.tasks.spawn(async move {
            let result = backend::run(spec, context)
                .await
                .map_err(|error| error.with_source_run(id));
            let _ = tx.send(DriverEvent::Finished { id, result });
        });
        self.owners.insert(task.id(), TaskOwner::Run(id));
    }
    fn request(&mut self, request: Request) {
        match request {
            Request::Terminal(request, attach_output, reply) => {
                let result = self.start_terminal(request, attach_output);
                let _ = reply.send(result);
            }
            Request::Tunnel(request, reply) => {
                let result = self.start_tunnel(request);
                let _ = reply.send(result);
            }
            Request::Command(request, cancel, attach_output, reply) => {
                self.start_command(request, cancel, attach_output, reply)
            }
            Request::Stop(id, reply) => self.stop(id, Some(reply), None),
            Request::Lifetime(id, policy, reply) => {
                let result = self.lifetime(id, policy);
                let _ = reply.send(result);
            }
            Request::Continue(id, reply) => {
                let result = self.continue_dependency(id);
                let _ = reply.send(result);
            }
            Request::Input(target, data, reply) => self.input(target, data, reply),
            Request::Resize(target, size, reply) => self.resize(target, size, reply),
            Request::Output(target, reply) => {
                let result = self
                    .entries
                    .get(&io_id(target))
                    .and_then(|e| e.output.as_ref())
                    .map(|s| s.buffer.subscribe())
                    .ok_or_else(not_found);
                let _ = reply.send(result);
            }
            Request::Options(options, reply) => {
                let result = options.validate().and_then(|()| {
                    if options.control_queue_capacity != self.options.control_queue_capacity {
                        return Err(TerminalError::invalid(
                            "control queue capacity is fixed for a service lifetime",
                        ));
                    }
                    for entry in self.entries.values() {
                        if let Some(output) = &entry.output {
                            output.buffer.resize(options.output_buffer_bytes);
                        }
                    }
                    self.shared.view.lock().unwrap().options = options.clone();
                    self.options = options;
                    Ok(())
                });
                let _ = reply.send(result);
            }
            Request::Shutdown(reply) => self.begin_shutdown(reply),
            Request::File(request, reply) => {
                let result = self.start_file(request);
                let _ = reply.send(result);
            }
            Request::StopFile(id, reply) => self.stop_file(id, Some(reply)),
            Request::List(id, path, cancel, reply) => self.list(id, path, cancel, reply),
            Request::Mkdir(id, path, reply) => self.mkdir(id, path, reply),
            Request::Rename(id, source, destination, reply) => {
                self.rename(id, source, destination, reply)
            }
        }
    }
    fn input(&mut self, target: TerminalIoTarget, data: Vec<u8>, reply: Reply<()>) {
        let Some(entry) = self.entries.get(&io_id(target)) else {
            let _ = reply.send(Err(not_found()));
            return;
        };
        if !matches!(entry.phase, Phase::Running) {
            let _ = reply.send(Err(invalid_state()));
            return;
        }
        if let Err(error) = entry.input.try_send(Input::Write(data, reply)) {
            let Input::Write(_, reply) = error.into_inner() else {
                unreachable!()
            };
            let _ = reply.send(Err(TerminalError::new(
                TerminalErrorKind::InvalidState,
                "terminal input queue is unavailable or full",
            )));
        }
    }
    fn resize(&mut self, target: TerminalIoTarget, size: TerminalSize, reply: Reply<()>) {
        if size.rows == 0 || size.cols == 0 {
            let _ = reply.send(Err(TerminalError::invalid(
                "terminal size must be positive",
            )));
            return;
        }
        let Some(entry) = self.entries.get(&io_id(target)) else {
            let _ = reply.send(Err(not_found()));
            return;
        };
        if entry.stopping() {
            let _ = reply.send(Err(invalid_state()));
            return;
        }
        *entry.size.lock().unwrap() = Some(size);
        if matches!(entry.phase, Phase::Running) {
            if let Err(error) = entry.input.try_send(Input::Resize(size, reply)) {
                let Input::Resize(_, reply) = error.into_inner() else {
                    unreachable!()
                };
                let _ = reply.send(Err(invalid_state()));
            }
        } else {
            let _ = reply.send(Ok(()));
        }
    }
}
fn io_id(id: TerminalIoTarget) -> RunId {
    match id {
        TerminalIoTarget::Terminal(id) => RunId::Terminal(id),
        TerminalIoTarget::Command(id) => RunId::Command(id),
    }
}
fn not_found() -> TerminalError {
    TerminalError::new(TerminalErrorKind::NotFound, "active run not found")
}
fn invalid_state() -> TerminalError {
    TerminalError::new(
        TerminalErrorKind::InvalidState,
        "operation is not valid in the current state",
    )
}
