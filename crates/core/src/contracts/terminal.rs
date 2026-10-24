// Stable value and capability contracts; no executable service owner or handle.
#[allow(unused_imports)]
pub(crate) use nooboard_terminal::{
    BoxFuture, CancellationToken, CommandConfigId, CommandLifetimePolicy, CommandReusePolicy,
    CommandRun, CommandRunId, CommandRunResult, CommandRunState, CommandStartSpec,
    CommandStartTarget, DependencyGateState, DependencyOwnerRunId, DependencyReleaseReason,
    DependencyRun, DependencyRunId, DependencyStartSpec, DeviceId, FileCleanupIssue,
    FileConflictPolicy, FileOperationPath, FileOperationProgress, FileOperationRun,
    FileOperationRunId, FileOperationRunResult, FileOperationRunState, FileOperationSpec,
    FileTransferItem, RemoteDirectoryEntry, RemoteDirectoryListing, RemoteEntryKind, RemotePath,
    RunId, RunLease, RunLeaseProvider, RunTarget, SshAddressSource, SshAuthKind, SshAuthMaterial,
    SshAuthProvider, SshConnectOptions, SshConnectionOwnerRunId, SshConnectionProvider,
    SshHostVerificationRequest, SshHostVerifier, SshPublicKey, SshRunTarget, SshStartTarget,
    SshTargetConfigId, SshTargetSummary, StartCommandRequest, StartFileOperationRequest,
    StartTerminalRequest, StartTunnelRequest, StopMethod, TerminalConfigId,
    TerminalConfigReference, TerminalError, TerminalErrorKind, TerminalEvent, TerminalEventStream,
    TerminalIoOptions, TerminalIoTarget, TerminalOutputChunk, TerminalOutputError,
    TerminalOutputStream, TerminalOutputSubscription, TerminalRun, TerminalRunId,
    TerminalRunResult, TerminalRunState, TerminalServiceOptions, TerminalShutdownError,
    TerminalSize, TerminalSnapshot, TerminalSnapshotStream, TerminalStartTarget,
    TerminalStopOptions, TerminalSubscription, TerminalSubscriptionError, TunnelConfigId,
    TunnelEndpoint, TunnelForwardConfig, TunnelRun, TunnelRunId, TunnelRunResult, TunnelRunState,
};
