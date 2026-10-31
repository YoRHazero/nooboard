use super::providers::Providers;
use crate::{
    contracts::{PortFuture, terminal::*},
    features::terminal::{
        domain::config::{StartKind, StartPlan},
        ports::engine::{TerminalEngine, TerminalEngineOwner},
    },
};
use std::sync::Arc;
pub(crate) struct EngineAdapter {
    pub handle: nooboard_terminal::Terminal,
    pub providers: Arc<Providers>,
}
pub(crate) struct EngineOwner(pub nooboard_terminal::TerminalService);
impl TerminalEngineOwner for EngineOwner {
    fn shutdown(&mut self) -> PortFuture<'_, ()> {
        Box::pin(async move { self.0.shutdown().await.map_err(Into::into) })
    }
}
impl EngineAdapter {
    async fn start_terminal(
        &self,
        request: StartTerminalRequest,
        output: bool,
    ) -> crate::Result<(RunId, Option<TerminalOutputSubscription>)> {
        if output {
            let (id, stream) = self.handle.start_terminal_with_output(request).await?;
            Ok((RunId::Terminal(id), Some(stream)))
        } else {
            self.handle
                .start_terminal(request)
                .await
                .map(|id| (RunId::Terminal(id), None))
                .map_err(Into::into)
        }
    }
}
impl TerminalEngine for EngineAdapter {
    fn is_available(&self) -> bool {
        self.handle.is_available()
    }
    fn snapshot(&self) -> TerminalSnapshot {
        self.handle.snapshot()
    }
    fn subscribe(&self) -> TerminalSubscription {
        self.handle.subscribe()
    }
    fn start(
        &self,
        plan: StartPlan,
        cancellation: CancellationToken,
        output: bool,
    ) -> PortFuture<'_, (RunId, Option<TerminalOutputSubscription>)> {
        Box::pin(async move {
            match plan.root {
                StartKind::LocalTerminal(name) => {
                    self.start_terminal(
                        StartTerminalRequest {
                            config_id: None,
                            name,
                            target: TerminalStartTarget::Local,
                            dependencies: vec![],
                        },
                        output,
                    )
                    .await
                }
                StartKind::Terminal(id) => {
                    self.start_terminal(self.providers.terminal(&plan.catalog, id).await?, output)
                        .await
                }
                StartKind::Command(id, reuse_policy) => {
                    let request = StartCommandRequest {
                        command: self.providers.command(&plan.catalog, id).await?,
                        reuse_policy,
                    };
                    if output {
                        let (id, stream) = self
                            .handle
                            .start_command_with_output(request, cancellation)
                            .await?;
                        Ok((RunId::Command(id), Some(stream)))
                    } else {
                        self.handle
                            .start_command(request, cancellation)
                            .await
                            .map(|id| (RunId::Command(id), None))
                            .map_err(Into::into)
                    }
                }
                StartKind::Tunnel(id) => self
                    .handle
                    .start_tunnel(self.providers.tunnel(&plan.catalog, id).await?)
                    .await
                    .map(|id| (RunId::Tunnel(id), None))
                    .map_err(Into::into),
            }
        })
    }
    fn stop_terminal(&self, id: TerminalRunId) -> PortFuture<'_, ()> {
        Box::pin(async move { self.handle.stop_terminal(id).await.map_err(Into::into) })
    }
    fn stop_command(&self, id: CommandRunId) -> PortFuture<'_, ()> {
        Box::pin(async move { self.handle.stop_command(id).await.map_err(Into::into) })
    }
    fn stop_tunnel(&self, id: TunnelRunId) -> PortFuture<'_, ()> {
        Box::pin(async move { self.handle.stop_tunnel(id).await.map_err(Into::into) })
    }
    fn set_command_lifetime_policy(
        &self,
        id: CommandRunId,
        policy: CommandLifetimePolicy,
    ) -> PortFuture<'_, ()> {
        Box::pin(async move {
            self.handle
                .set_command_lifetime_policy(id, policy)
                .await
                .map_err(Into::into)
        })
    }
    fn continue_dependency(&self, id: DependencyRunId) -> PortFuture<'_, ()> {
        Box::pin(async move {
            self.handle
                .continue_dependency(id)
                .await
                .map_err(Into::into)
        })
    }
    fn write_input(&self, target: TerminalIoTarget, data: Vec<u8>) -> PortFuture<'_, ()> {
        Box::pin(async move {
            self.handle
                .write_input(target, data)
                .await
                .map_err(Into::into)
        })
    }
    fn resize(&self, target: TerminalIoTarget, size: TerminalSize) -> PortFuture<'_, ()> {
        Box::pin(async move { self.handle.resize(target, size).await.map_err(Into::into) })
    }
    fn subscribe_output(
        &self,
        target: TerminalIoTarget,
    ) -> PortFuture<'_, TerminalOutputSubscription> {
        Box::pin(async move {
            self.handle
                .subscribe_output(target)
                .await
                .map_err(Into::into)
        })
    }
    fn list_directory(
        &self,
        id: TerminalRunId,
        path: Option<RemotePath>,
        cancellation: CancellationToken,
    ) -> PortFuture<'_, RemoteDirectoryListing> {
        Box::pin(async move {
            self.handle
                .list_directory(id, path, cancellation)
                .await
                .map_err(Into::into)
        })
    }
    fn start_file_operation(
        &self,
        request: StartFileOperationRequest,
    ) -> PortFuture<'_, FileOperationRunId> {
        Box::pin(async move {
            self.handle
                .start_file_operation(request)
                .await
                .map_err(Into::into)
        })
    }
    fn stop_file_operation(&self, id: FileOperationRunId) -> PortFuture<'_, ()> {
        Box::pin(async move {
            self.handle
                .stop_file_operation(id)
                .await
                .map_err(Into::into)
        })
    }
    fn create_directory(&self, id: TerminalRunId, path: RemotePath) -> PortFuture<'_, ()> {
        Box::pin(async move {
            self.handle
                .create_directory(id, path)
                .await
                .map_err(Into::into)
        })
    }
    fn rename_path(
        &self,
        id: TerminalRunId,
        source: RemotePath,
        destination: RemotePath,
    ) -> PortFuture<'_, ()> {
        Box::pin(async move {
            self.handle
                .rename_path(id, source, destination)
                .await
                .map_err(Into::into)
        })
    }
    fn update_options(&self, options: TerminalServiceOptions) -> PortFuture<'_, ()> {
        Box::pin(async move {
            self.handle
                .update_options(options)
                .await
                .map_err(Into::into)
        })
    }
}
