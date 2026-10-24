use crate::{
    contracts::{PortFuture, terminal::*},
    features::terminal::domain::config::StartPlan,
};
pub(crate) trait TerminalEngine: Send + Sync {
    fn is_available(&self) -> bool;
    fn snapshot(&self) -> TerminalSnapshot;
    fn subscribe(&self) -> TerminalSubscription;
    fn start(
        &self,
        plan: StartPlan,
        cancellation: CancellationToken,
        output: bool,
    ) -> PortFuture<'_, (RunId, Option<TerminalOutputSubscription>)>;
    fn stop_terminal(&self, id: TerminalRunId) -> PortFuture<'_, ()>;
    fn stop_command(&self, id: CommandRunId) -> PortFuture<'_, ()>;
    fn stop_tunnel(&self, id: TunnelRunId) -> PortFuture<'_, ()>;
    fn set_command_lifetime_policy(
        &self,
        id: CommandRunId,
        policy: CommandLifetimePolicy,
    ) -> PortFuture<'_, ()>;
    fn continue_dependency(&self, id: DependencyRunId) -> PortFuture<'_, ()>;
    fn write_input(&self, target: TerminalIoTarget, data: Vec<u8>) -> PortFuture<'_, ()>;
    fn resize(&self, target: TerminalIoTarget, size: TerminalSize) -> PortFuture<'_, ()>;
    fn subscribe_output(
        &self,
        target: TerminalIoTarget,
    ) -> PortFuture<'_, TerminalOutputSubscription>;
    fn list_directory(
        &self,
        id: TerminalRunId,
        path: Option<RemotePath>,
        cancellation: CancellationToken,
    ) -> PortFuture<'_, RemoteDirectoryListing>;
    fn start_file_operation(
        &self,
        request: StartFileOperationRequest,
    ) -> PortFuture<'_, FileOperationRunId>;
    fn stop_file_operation(&self, id: FileOperationRunId) -> PortFuture<'_, ()>;
    fn create_directory(&self, id: TerminalRunId, path: RemotePath) -> PortFuture<'_, ()>;
    fn rename_path(
        &self,
        id: TerminalRunId,
        source: RemotePath,
        destination: RemotePath,
    ) -> PortFuture<'_, ()>;
    fn update_options(&self, options: TerminalServiceOptions) -> PortFuture<'_, ()>;
}
pub(crate) trait TerminalEngineOwner: Send {
    fn shutdown(&mut self) -> PortFuture<'_, ()>;
}
