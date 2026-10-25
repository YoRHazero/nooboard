use super::*;
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, path::PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SshAddressSource {
    Host(String),
    Device(DeviceId),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SshTargetSummary {
    pub address: SshAddressSource,
    pub port: u16,
    pub username: String,
    pub auth_kind: SshAuthKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Copy, Hash)]
pub enum SshAuthKind {
    Password,
    PrivateKey,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Copy, Hash)]
pub enum CommandReusePolicy {
    Shared,
    AlwaysNew,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Copy, Hash)]
pub enum CommandLifetimePolicy {
    Dependent,
    Independent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TunnelForwardConfig {
    Local {
        listen: TunnelEndpoint,
        destination: TunnelEndpoint,
    },
    Remote {
        listen: TunnelEndpoint,
        destination: TunnelEndpoint,
    },
    Dynamic {
        listen: TunnelEndpoint,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TunnelEndpoint {
    pub host: String,
    pub port: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Copy, Hash)]
pub enum DependencyReleaseReason {
    SuccessfulExit,
    ReadyMarker,
    ManualContinue,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunTarget {
    Local,
    Ssh(SshRunTarget),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SshRunTarget {
    pub config_id: SshTargetConfigId,
    pub config_name: String,
    pub configured: SshTargetSummary,
    pub resolved_address: Option<SocketAddr>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandRun {
    pub id: CommandRunId,
    pub config_id: CommandConfigId,
    pub config_name: String,
    pub target: RunTarget,
    pub command: String,
    pub lifetime_policy: CommandLifetimePolicy,
    pub state: CommandRunState,
    pub stop_method: Option<StopMethod>,
    pub ready_marker_received: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandRunState {
    Starting,
    Running,
    Stopping { error: Option<TerminalError> },
    Ended { result: CommandRunResult },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandRunResult {
    Succeeded,
    Failed(TerminalError),
    Stopped,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyRun {
    pub id: DependencyRunId,
    pub owner_run_id: DependencyOwnerRunId,
    pub command_config_id: CommandConfigId,
    pub reuse_policy: CommandReusePolicy,
    pub command_run_id: Option<CommandRunId>,
    pub gate_state: DependencyGateState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Copy, Hash)]
pub enum DependencyOwnerRunId {
    Terminal(TerminalRunId),
    Tunnel(TunnelRunId),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencyGateState {
    Pending,
    WaitingForInstance,
    WaitingForRelease,
    Released { reason: DependencyReleaseReason },
    Failed { error: TerminalError },
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalRun {
    pub id: TerminalRunId,
    pub config_id: Option<TerminalConfigId>,
    pub name: String,
    pub target: RunTarget,
    pub dependency_run_ids: Vec<DependencyRunId>,
    pub state: TerminalRunState,
    pub stop_method: Option<StopMethod>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalRunState {
    WaitingForDependencies,
    Starting,
    Running,
    Stopping { error: Option<TerminalError> },
    Ended { result: TerminalRunResult },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalRunResult {
    Succeeded,
    Failed(TerminalError),
    Stopped,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TunnelRun {
    pub id: TunnelRunId,
    pub config_id: TunnelConfigId,
    pub config_name: String,
    pub target: SshRunTarget,
    pub forward: TunnelForwardConfig,
    pub dependency_run_ids: Vec<DependencyRunId>,
    pub state: TunnelRunState,
    pub stop_method: Option<StopMethod>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TunnelRunState {
    WaitingForDependencies,
    Starting,
    Running,
    Stopping { error: Option<TerminalError> },
    Ended { result: TunnelRunResult },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TunnelRunResult {
    Failed(TerminalError),
    Stopped,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TerminalSnapshot {
    pub terminal_runs: Vec<TerminalRun>,
    pub command_runs: Vec<CommandRun>,
    pub tunnel_runs: Vec<TunnelRun>,
    pub dependency_runs: Vec<DependencyRun>,
    pub file_operation_runs: Vec<FileOperationRun>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalEvent {
    TerminalEnded { run: TerminalRun },
    CommandEnded { run: CommandRun },
    TunnelEnded { run: TunnelRun },
    FileOperationEnded { run: FileOperationRun },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalSubscriptionError {
    Lagged,
    Closed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Copy, Hash)]
pub enum TerminalIoTarget {
    Terminal(TerminalRunId),
    Command(CommandRunId),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Copy, Hash)]
pub struct TerminalSize {
    pub rows: u16,
    pub cols: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalOutputChunk {
    pub offset: u64,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalOutputError {
    Lagged {
        expected_offset: u64,
        available_offset: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteDirectoryListing {
    pub path: RemotePath,
    pub entries: Vec<RemoteDirectoryEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteDirectoryEntry {
    pub name: String,
    pub kind: RemoteEntryKind,
    pub size: Option<u64>,
    pub modified_at: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Copy, Hash)]
pub enum RemoteEntryKind {
    File,
    Directory,
    SymbolicLink,
    Other,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartFileOperationRequest {
    pub terminal_run_id: TerminalRunId,
    pub operation: FileOperationSpec,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileOperationSpec {
    Upload {
        items: Vec<FileTransferItem>,
        conflict_policy: FileConflictPolicy,
    },
    Download {
        items: Vec<FileTransferItem>,
        conflict_policy: FileConflictPolicy,
    },
    Delete {
        paths: Vec<RemotePath>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileTransferItem {
    pub local_path: PathBuf,
    pub remote_path: RemotePath,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Copy, Hash)]
pub enum FileConflictPolicy {
    Fail,
    Skip,
    Overwrite,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileOperationRun {
    pub id: FileOperationRunId,
    pub terminal_run_id: TerminalRunId,
    pub operation: FileOperationSpec,
    pub state: FileOperationRunState,
    pub stop_method: Option<StopMethod>,
    pub progress: FileOperationProgress,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileOperationRunState {
    Starting,
    Running,
    Stopping {
        error: Option<TerminalError>,
    },
    Ended {
        result: FileOperationRunResult,
        cleanup_issues: Vec<FileCleanupIssue>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileOperationRunResult {
    Succeeded,
    Failed(TerminalError),
    Stopped,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FileOperationProgress {
    pub completed_entries: u64,
    pub skipped_entries: u64,
    pub total_entries: Option<u64>,
    pub transferred_bytes: u64,
    pub total_bytes: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileOperationPath {
    Local(PathBuf),
    Remote(RemotePath),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileCleanupIssue {
    pub path: FileOperationPath,
    pub error: TerminalError,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Copy, Hash)]
pub enum StopMethod {
    Graceful,
    Forced,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Copy, Hash)]
pub enum RunId {
    Terminal(TerminalRunId),
    Command(CommandRunId),
    Tunnel(TunnelRunId),
    FileOperation(FileOperationRunId),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalShutdownError {
    pub pending_runs: Vec<RunId>,
    pub errors: Vec<TerminalError>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalError {
    pub kind: TerminalErrorKind,
    pub message: String,
    pub source_run_id: Option<RunId>,
    pub path: Option<FileOperationPath>,
    pub cause: Option<Box<TerminalError>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Copy, Hash)]
pub enum TerminalErrorKind {
    InvalidRequest,
    NotFound,
    InvalidState,
    ServiceShuttingDown,
    AddressUnavailable,
    HostKeyRejected,
    HostKeyMismatch,
    AuthenticationFailed,
    ConnectionFailed,
    ConnectionLost,
    ProcessFailed,
    DependencyFailed,
    StopFailed,
    StopTimedOut,
    PermissionDenied,
    PathNotFound,
    Conflict,
    Unsupported,
    Cancelled,
    CommitUnconfirmed,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Copy, Hash)]
pub enum SshConnectionOwnerRunId {
    Terminal(TerminalRunId),
    Command(CommandRunId),
    Tunnel(TunnelRunId),
}
