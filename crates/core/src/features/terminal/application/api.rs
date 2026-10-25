use super::requests::{Coordinator, Request};
use crate::{
    Error, Result,
    contracts::terminal::*,
    features::terminal::{
        domain::{config::StartKind, inputs::*, views::*},
        ports::engine::TerminalEngine,
    },
};
use std::sync::Arc;
use tokio::sync::watch;
#[derive(Clone)]
pub struct TerminalWorkspace {
    pub(crate) coordinator: Coordinator,
    pub(crate) engine: Arc<dyn TerminalEngine>,
    pub(crate) state: watch::Receiver<TerminalWorkspaceSnapshot>,
}
impl TerminalWorkspace {
    pub fn snapshot(&self) -> TerminalWorkspaceSnapshot {
        let mut view = self.state.borrow().clone();
        view.runs = self.engine.snapshot();
        view
    }
    pub fn subscribe(&self) -> watch::Receiver<TerminalWorkspaceSnapshot> {
        self.state.clone()
    }
    pub fn subscribe_runs(&self) -> TerminalSubscription {
        self.engine.subscribe()
    }
    pub async fn create_ssh_target(&self, input: SshTargetInput) -> Result<SshTargetConfigId> {
        self.coordinator
            .call(|r| Request::Ssh(None, input, r))
            .await
    }
    pub async fn update_ssh_target(
        &self,
        id: SshTargetConfigId,
        input: SshTargetInput,
    ) -> Result<()> {
        self.coordinator
            .call(|r| Request::Ssh(Some(id), input, r))
            .await
            .map(|_| ())
    }
    pub async fn create_command(&self, input: CommandInput) -> Result<CommandConfigId> {
        self.coordinator
            .call(|r| Request::Command(None, input, r))
            .await
    }
    pub async fn update_command(&self, id: CommandConfigId, input: CommandInput) -> Result<()> {
        self.coordinator
            .call(|r| Request::Command(Some(id), input, r))
            .await
            .map(|_| ())
    }
    pub async fn create_terminal(&self, input: TerminalInput) -> Result<TerminalConfigId> {
        self.coordinator
            .call(|r| Request::Terminal(None, input, r))
            .await
    }
    pub async fn update_terminal(&self, id: TerminalConfigId, input: TerminalInput) -> Result<()> {
        self.coordinator
            .call(|r| Request::Terminal(Some(id), input, r))
            .await
            .map(|_| ())
    }
    pub async fn create_tunnel(&self, input: TunnelInput) -> Result<TunnelConfigId> {
        self.coordinator
            .call(|r| Request::Tunnel(None, input, r))
            .await
    }
    pub async fn update_tunnel(&self, id: TunnelConfigId, input: TunnelInput) -> Result<()> {
        self.coordinator
            .call(|r| Request::Tunnel(Some(id), input, r))
            .await
            .map(|_| ())
    }

    pub async fn copy_config(
        &self,
        id: TerminalConfigReference,
    ) -> Result<TerminalConfigReference> {
        self.coordinator.call(|r| Request::Copy(id, r)).await
    }
    pub async fn delete_config(&self, id: TerminalConfigReference) -> Result<()> {
        self.coordinator.call(|r| Request::Delete(id, r)).await
    }
    pub async fn update_options(&self, options: TerminalServiceOptions) -> Result<()> {
        self.coordinator
            .call(|r| Request::Options(options, r))
            .await
    }
    pub async fn confirm_host(&self, id: TerminalPromptId, accept: bool) -> Result<()> {
        self.coordinator
            .call(|r| Request::ConfirmHost(id, accept, r))
            .await
    }
    pub async fn answer_password(
        &self,
        id: TerminalPromptId,
        password: SecretInput,
        remember: bool,
    ) -> Result<()> {
        self.coordinator
            .call(|r| Request::AnswerPassword(id, password, remember, r))
            .await
    }
    pub async fn cancel_prompt(&self, id: TerminalPromptId) -> Result<()> {
        self.coordinator
            .call(|r| Request::CancelPrompt(id, r))
            .await
    }
    pub async fn replace_host_key(
        &self,
        id: SshTargetConfigId,
        expected: Option<SshPublicKey>,
        replacement: SshPublicKey,
    ) -> Result<()> {
        self.coordinator
            .call(|r| Request::ReplaceHost(id, expected, replacement, r))
            .await
    }
    async fn start(
        &self,
        kind: StartKind,
        cancel: CancellationToken,
        output: bool,
    ) -> Result<(RunId, Option<TerminalOutputSubscription>)> {
        let guard = cancel.clone().drop_guard();
        let result = self
            .coordinator
            .call(|r| Request::Start(kind, cancel, output, r))
            .await;
        guard.disarm();
        result
    }
    /// Open a local shell without creating or occupying a saved configuration.
    pub async fn start_local_terminal(&self, name: String) -> Result<TerminalRunId> {
        match self
            .start(
                StartKind::LocalTerminal(name),
                CancellationToken::new(),
                false,
            )
            .await?
            .0
        {
            RunId::Terminal(id) => Ok(id),
            _ => Err(Error::Internal),
        }
    }
    /// Start and subscribe atomically, including output produced before the reply.
    pub async fn start_local_terminal_with_output(
        &self,
        name: String,
    ) -> Result<(TerminalRunId, TerminalOutputSubscription)> {
        match self
            .start(
                StartKind::LocalTerminal(name),
                CancellationToken::new(),
                true,
            )
            .await?
        {
            (RunId::Terminal(id), Some(output)) => Ok((id, output)),
            _ => Err(Error::Internal),
        }
    }
    pub async fn start_terminal(&self, id: TerminalConfigId) -> Result<TerminalRunId> {
        match self
            .start(StartKind::Terminal(id), CancellationToken::new(), false)
            .await?
            .0
        {
            RunId::Terminal(id) => Ok(id),
            _ => Err(Error::Internal),
        }
    }
    pub async fn start_terminal_with_output(
        &self,
        id: TerminalConfigId,
    ) -> Result<(TerminalRunId, TerminalOutputSubscription)> {
        match self
            .start(StartKind::Terminal(id), CancellationToken::new(), true)
            .await?
        {
            (RunId::Terminal(id), Some(output)) => Ok((id, output)),
            _ => Err(Error::Internal),
        }
    }
    pub async fn start_command(
        &self,
        id: CommandConfigId,
        reuse: CommandReusePolicy,
        cancel: CancellationToken,
    ) -> Result<CommandRunId> {
        match self
            .start(StartKind::Command(id, reuse), cancel, false)
            .await?
            .0
        {
            RunId::Command(id) => Ok(id),
            _ => Err(Error::Internal),
        }
    }
    pub async fn start_command_with_output(
        &self,
        id: CommandConfigId,
        reuse: CommandReusePolicy,
        cancel: CancellationToken,
    ) -> Result<(CommandRunId, TerminalOutputSubscription)> {
        match self
            .start(StartKind::Command(id, reuse), cancel, true)
            .await?
        {
            (RunId::Command(id), Some(output)) => Ok((id, output)),
            _ => Err(Error::Internal),
        }
    }
    pub async fn start_tunnel(&self, id: TunnelConfigId) -> Result<TunnelRunId> {
        match self
            .start(StartKind::Tunnel(id), CancellationToken::new(), false)
            .await?
            .0
        {
            RunId::Tunnel(id) => Ok(id),
            _ => Err(Error::Internal),
        }
    }
    pub async fn stop_terminal(&self, id: TerminalRunId) -> Result<()> {
        self.engine.stop_terminal(id).await
    }
    pub async fn stop_command(&self, id: CommandRunId) -> Result<()> {
        self.engine.stop_command(id).await
    }
    pub async fn stop_tunnel(&self, id: TunnelRunId) -> Result<()> {
        self.engine.stop_tunnel(id).await
    }
    pub async fn set_command_lifetime_policy(
        &self,
        id: CommandRunId,
        policy: CommandLifetimePolicy,
    ) -> Result<()> {
        self.engine.set_command_lifetime_policy(id, policy).await
    }
    pub async fn continue_dependency(&self, id: DependencyRunId) -> Result<()> {
        self.engine.continue_dependency(id).await
    }
    pub async fn write_input(&self, target: TerminalIoTarget, data: Vec<u8>) -> Result<()> {
        self.engine.write_input(target, data).await
    }
    pub async fn resize(&self, target: TerminalIoTarget, size: TerminalSize) -> Result<()> {
        self.engine.resize(target, size).await
    }
    pub async fn subscribe_output(
        &self,
        target: TerminalIoTarget,
    ) -> Result<TerminalOutputSubscription> {
        self.engine.subscribe_output(target).await
    }
    pub async fn list_directory(
        &self,
        id: TerminalRunId,
        path: Option<RemotePath>,
        cancellation: CancellationToken,
    ) -> Result<RemoteDirectoryListing> {
        self.engine.list_directory(id, path, cancellation).await
    }
    pub async fn start_file_operation(
        &self,
        request: StartFileOperationRequest,
    ) -> Result<FileOperationRunId> {
        self.engine.start_file_operation(request).await
    }
    pub async fn stop_file_operation(&self, id: FileOperationRunId) -> Result<()> {
        self.engine.stop_file_operation(id).await
    }
    pub async fn create_directory(&self, id: TerminalRunId, path: RemotePath) -> Result<()> {
        self.engine.create_directory(id, path).await
    }
    pub async fn rename_path(
        &self,
        id: TerminalRunId,
        source: RemotePath,
        destination: RemotePath,
    ) -> Result<()> {
        self.engine.rename_path(id, source, destination).await
    }
}
