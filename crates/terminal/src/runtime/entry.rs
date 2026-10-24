use super::*;
use crate::backend::{DriverSpec, Input, OutputSink};

#[derive(Clone)]
pub(super) enum Phase {
    Waiting,
    Starting,
    Running,
    Stopping(Option<TerminalError>),
}
pub(super) enum Kind {
    Terminal(TerminalConfigId),
    Command {
        config_id: CommandConfigId,
        command: String,
        lifetime: CommandLifetimePolicy,
        ready: bool,
    },
    Tunnel {
        config_id: TunnelConfigId,
        forward: TunnelForwardConfig,
    },
}
pub(super) struct Entry {
    pub _config_lease: Option<Box<dyn RunLease>>,
    pub id: RunId,
    pub order: u64,
    pub name: String,
    pub target: RunTarget,
    pub kind: Kind,
    pub phase: Phase,
    pub dependencies: Vec<DependencyRunId>,
    pub driver: Option<DriverSpec>,
    pub input_rx: Option<mpsc::Receiver<Input>>,
    pub input: mpsc::Sender<Input>,
    pub output: Option<Arc<OutputSink>>,
    pub size: Arc<Mutex<Option<TerminalSize>>>,
    pub stop: CancellationToken,
    pub force: CancellationToken,
    pub children_drained: CancellationToken,
    pub connection: Option<Arc<backend::ssh::Connection>>,
    pub transport: Arc<Mutex<Option<Arc<backend::ssh::TransportControl>>>>,
    pub launched: bool,
    pub cleaning: bool,
    pub stop_requested: bool,
    pub ending_error: Option<TerminalError>,
    pub method: Option<StopMethod>,
    pub waiters: Vec<Reply<()>>,
    pub finished: Option<Result<()>>,
}
impl Entry {
    pub fn stopping(&self) -> bool {
        matches!(self.phase, Phase::Stopping(_))
    }
    pub fn owner(&self) -> Option<DependencyOwnerRunId> {
        match self.id {
            RunId::Terminal(id) => Some(DependencyOwnerRunId::Terminal(id)),
            RunId::Tunnel(id) => Some(DependencyOwnerRunId::Tunnel(id)),
            _ => None,
        }
    }
    pub fn command_config(&self) -> Option<CommandConfigId> {
        match self.kind {
            Kind::Command { config_id, .. } => Some(config_id),
            _ => None,
        }
    }
    pub fn terminal(&self) -> TerminalRun {
        let (RunId::Terminal(id), Kind::Terminal(config_id)) = (&self.id, &self.kind) else {
            unreachable!()
        };
        TerminalRun {
            id: *id,
            config_id: *config_id,
            config_name: self.name.clone(),
            target: self.target.clone(),
            dependency_run_ids: self.dependencies.clone(),
            stop_method: self.method,
            state: match &self.phase {
                Phase::Waiting => TerminalRunState::WaitingForDependencies,
                Phase::Starting => TerminalRunState::Starting,
                Phase::Running => TerminalRunState::Running,
                Phase::Stopping(error) => TerminalRunState::Stopping {
                    error: error.clone(),
                },
            },
        }
    }
    pub fn command(&self) -> CommandRun {
        let (
            RunId::Command(id),
            Kind::Command {
                config_id,
                command,
                lifetime,
                ready,
            },
        ) = (&self.id, &self.kind)
        else {
            unreachable!()
        };
        CommandRun {
            id: *id,
            config_id: *config_id,
            config_name: self.name.clone(),
            target: self.target.clone(),
            command: command.clone(),
            lifetime_policy: *lifetime,
            ready_marker_received: *ready,
            stop_method: self.method,
            state: match &self.phase {
                Phase::Waiting | Phase::Starting => CommandRunState::Starting,
                Phase::Running => CommandRunState::Running,
                Phase::Stopping(error) => CommandRunState::Stopping {
                    error: error.clone(),
                },
            },
        }
    }
    pub fn tunnel(&self) -> TunnelRun {
        let (RunId::Tunnel(id), Kind::Tunnel { config_id, forward }, RunTarget::Ssh(target)) =
            (&self.id, &self.kind, &self.target)
        else {
            unreachable!()
        };
        TunnelRun {
            id: *id,
            config_id: *config_id,
            config_name: self.name.clone(),
            target: target.clone(),
            forward: forward.clone(),
            dependency_run_ids: self.dependencies.clone(),
            stop_method: self.method,
            state: match &self.phase {
                Phase::Waiting => TunnelRunState::WaitingForDependencies,
                Phase::Starting => TunnelRunState::Starting,
                Phase::Running => TunnelRunState::Running,
                Phase::Stopping(error) => TunnelRunState::Stopping {
                    error: error.clone(),
                },
            },
        }
    }
}
pub(super) struct Dependency {
    pub wait_order: Option<u64>,
    pub view: DependencyRun,
    pub spec: DependencyStartSpec,
}
pub(super) struct FileEntry {
    pub view: FileOperationRun,
    pub stop: CancellationToken,
    pub waiters: Vec<Reply<()>>,
}
pub(super) struct PendingCommand {
    pub order: u64,
    pub request: StartCommandRequest,
    pub cancel: CancellationToken,
    pub done: CancellationToken,
    pub attach_output: bool,
    pub reply: Reply<(CommandRunId, Option<TerminalOutputSubscription>)>,
}
pub(super) struct ShortOperation {
    pub owner: TerminalRunId,
    pub stop: CancellationToken,
}
pub(super) enum TaskOwner {
    Run(RunId),
    File(FileOperationRunId),
    Short(u64),
}
pub(super) enum Timer {
    Force(RunId),
    StopExpired(RunId),
    FileStopExpired(FileOperationRunId),
    PendingChanged,
    ForceAll,
}
pub(super) fn owner_id(id: DependencyOwnerRunId) -> RunId {
    match id {
        DependencyOwnerRunId::Terminal(id) => RunId::Terminal(id),
        DependencyOwnerRunId::Tunnel(id) => RunId::Tunnel(id),
    }
}
pub(super) fn ssh_target(target: &SshStartTarget) -> RunTarget {
    RunTarget::Ssh(SshRunTarget {
        config_id: target.config_id,
        config_name: target.config_name.clone(),
        configured: target.summary.clone(),
        resolved_address: None,
    })
}
