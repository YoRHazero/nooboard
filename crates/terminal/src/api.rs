//! Public service boundary. Protocol and operating-system handles remain private.
pub use crate::{model::*, options::*, secret::*};
pub use tokio_util::sync::CancellationToken;

pub use crate::output::{TerminalOutputStream, TerminalOutputSubscription};
pub use crate::subscription::{TerminalEventStream, TerminalSnapshotStream, TerminalSubscription};

use crate::{
    runtime::{Manager, Request},
    subscription::Shared,
};
use std::sync::Arc;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

pub struct TerminalServiceDependencies {
    pub host_verifier: Arc<dyn SshHostVerifier>,
}
/// Owns the management task. Explicit shutdown preserves ownership on failure.
pub struct TerminalService {
    terminal: Terminal,
    owner_stop: CancellationToken,
    task: Option<JoinHandle<()>>,
    manager_failure: Option<TerminalShutdownError>,
}
#[derive(Clone)]
pub struct Terminal {
    requests: mpsc::Sender<Request>,
    shared: Arc<Shared>,
}
impl TerminalService {
    pub fn start(
        options: TerminalServiceOptions,
        dependencies: TerminalServiceDependencies,
    ) -> Result<Self> {
        Self::start_inner(options, dependencies, None)
    }
    /// Compose reliable configuration occupancy without consuming advisory UI events.
    pub fn start_with_leases(
        options: TerminalServiceOptions,
        dependencies: TerminalServiceDependencies,
        leases: Arc<dyn RunLeaseProvider>,
    ) -> Result<Self> {
        Self::start_inner(options, dependencies, Some(leases))
    }
    fn start_inner(
        options: TerminalServiceOptions,
        dependencies: TerminalServiceDependencies,
        leases: Option<Arc<dyn RunLeaseProvider>>,
    ) -> Result<Self> {
        options.validate()?;
        let runtime = tokio::runtime::Handle::try_current()
            .map_err(|_| TerminalError::invalid("TerminalService requires a Tokio runtime"))?;
        let shared = Shared::new(options.clone());
        let (requests, inbox) = mpsc::channel(options.control_queue_capacity);
        let owner_stop = CancellationToken::new();
        let manager = Manager::new(
            shared.clone(),
            options,
            dependencies.host_verifier,
            inbox,
            owner_stop.clone(),
            leases,
        );
        let task = runtime.spawn(manager.run());
        Ok(Self {
            terminal: Terminal { requests, shared },
            owner_stop,
            task: Some(task),
            manager_failure: None,
        })
    }
    pub fn handle(&self) -> Terminal {
        self.terminal.clone()
    }
    pub async fn shutdown(&mut self) -> std::result::Result<(), TerminalShutdownError> {
        if let Some(error) = &self.manager_failure {
            return Err(error.clone());
        }
        if self.task.is_none() {
            return Ok(());
        }
        let duration = self
            .terminal
            .shared
            .view
            .lock()
            .unwrap()
            .options
            .stop
            .shutdown_timeout;
        let deadline = tokio::time::Instant::now() + duration;
        let operation = self.terminal.call(|reply| Request::Shutdown(Some(reply)));
        let failure = match tokio::time::timeout_at(deadline, operation).await {
            Ok(Ok(())) => None,
            Ok(Err(error)) if self.terminal.snapshot().is_empty() => {
                let _ = error;
                None
            }
            Ok(Err(error)) => Some(error),
            Err(_) => Some(TerminalError::new(
                TerminalErrorKind::StopTimedOut,
                "terminal service shutdown exceeded its budget",
            )),
        };
        if let Some(error) = failure {
            return Err(TerminalShutdownError {
                pending_runs: self.terminal.snapshot().active_ids(),
                errors: vec![error],
            });
        }
        if let Some(task) = self.task.as_mut() {
            match tokio::time::timeout_at(deadline, task).await {
                Ok(Ok(())) => {
                    self.task.take();
                }
                Ok(Err(error)) => {
                    self.task.take();
                    let error = TerminalShutdownError {
                        pending_runs: self.terminal.snapshot().active_ids(),
                        errors: vec![TerminalError::new(
                            TerminalErrorKind::StopFailed,
                            error.to_string(),
                        )],
                    };
                    self.manager_failure = Some(error.clone());
                    return Err(error);
                }
                Err(_) => {
                    return Err(TerminalShutdownError {
                        pending_runs: self.terminal.snapshot().active_ids(),
                        errors: vec![TerminalError::new(
                            TerminalErrorKind::StopTimedOut,
                            "terminal supervision has not finished",
                        )],
                    });
                }
            }
        }
        Ok(())
    }
}
impl Drop for TerminalService {
    fn drop(&mut self) {
        self.owner_stop.cancel();
    }
}
impl Terminal {
    pub fn is_available(&self) -> bool {
        !self.requests.is_closed()
    }
    async fn call<T>(&self, make: impl FnOnce(oneshot::Sender<Result<T>>) -> Request) -> Result<T> {
        let (reply, receive) = oneshot::channel();
        self.requests
            .send(make(reply))
            .await
            .map_err(|_| TerminalError::closed())?;
        receive.await.map_err(|_| TerminalError::closed())?
    }
    pub fn snapshot(&self) -> TerminalSnapshot {
        self.shared.view.lock().unwrap().snapshot.clone()
    }
    pub fn subscribe(&self) -> TerminalSubscription {
        self.shared.subscribe()
    }
    pub async fn update_options(&self, options: TerminalServiceOptions) -> Result<()> {
        self.call(|r| Request::Options(options, r)).await
    }
    pub async fn start_terminal(&self, request: StartTerminalRequest) -> Result<TerminalRunId> {
        self.call(|r| Request::Terminal(request, false, r))
            .await
            .map(|(id, _)| id)
    }
    pub async fn start_tunnel(&self, request: StartTunnelRequest) -> Result<TunnelRunId> {
        self.call(|r| Request::Tunnel(request, r)).await
    }
    pub async fn start_command(
        &self,
        request: StartCommandRequest,
        cancellation: CancellationToken,
    ) -> Result<CommandRunId> {
        self.call(|r| Request::Command(request, cancellation, false, r))
            .await
            .map(|(id, _)| id)
    }
    /// Attach output during admission so even an immediately exiting process can be observed.
    pub async fn start_terminal_with_output(
        &self,
        request: StartTerminalRequest,
    ) -> Result<(TerminalRunId, TerminalOutputSubscription)> {
        let (id, output) = self.call(|r| Request::Terminal(request, true, r)).await?;
        Ok((
            id,
            output.ok_or_else(|| {
                TerminalError::new(TerminalErrorKind::Other, "output was not attached")
            })?,
        ))
    }
    /// Shared acquisition and output attachment are one serialized operation.
    pub async fn start_command_with_output(
        &self,
        request: StartCommandRequest,
        cancellation: CancellationToken,
    ) -> Result<(CommandRunId, TerminalOutputSubscription)> {
        let (id, output) = self
            .call(|r| Request::Command(request, cancellation, true, r))
            .await?;
        Ok((
            id,
            output.ok_or_else(|| {
                TerminalError::new(TerminalErrorKind::Other, "output was not attached")
            })?,
        ))
    }
    pub async fn stop_terminal(&self, id: TerminalRunId) -> Result<()> {
        self.call(|r| Request::Stop(RunId::Terminal(id), r)).await
    }
    pub async fn stop_command(&self, id: CommandRunId) -> Result<()> {
        self.call(|r| Request::Stop(RunId::Command(id), r)).await
    }
    pub async fn stop_tunnel(&self, id: TunnelRunId) -> Result<()> {
        self.call(|r| Request::Stop(RunId::Tunnel(id), r)).await
    }
    pub async fn set_command_lifetime_policy(
        &self,
        id: CommandRunId,
        policy: CommandLifetimePolicy,
    ) -> Result<()> {
        self.call(|r| Request::Lifetime(id, policy, r)).await
    }
    pub async fn continue_dependency(&self, id: DependencyRunId) -> Result<()> {
        self.call(|r| Request::Continue(id, r)).await
    }
    pub async fn write_input(&self, target: TerminalIoTarget, data: Vec<u8>) -> Result<()> {
        self.call(|r| Request::Input(target, data, r)).await
    }
    pub async fn resize(&self, target: TerminalIoTarget, size: TerminalSize) -> Result<()> {
        self.call(|r| Request::Resize(target, size, r)).await
    }
    pub async fn subscribe_output(
        &self,
        target: TerminalIoTarget,
    ) -> Result<TerminalOutputSubscription> {
        self.call(|r| Request::Output(target, r)).await
    }
    pub async fn list_directory(
        &self,
        id: TerminalRunId,
        path: Option<RemotePath>,
        cancellation: CancellationToken,
    ) -> Result<RemoteDirectoryListing> {
        self.call(|r| Request::List(id, path, cancellation, r))
            .await
    }
    pub async fn start_file_operation(
        &self,
        request: StartFileOperationRequest,
    ) -> Result<FileOperationRunId> {
        self.call(|r| Request::File(request, r)).await
    }
    pub async fn stop_file_operation(&self, id: FileOperationRunId) -> Result<()> {
        self.call(|r| Request::StopFile(id, r)).await
    }
    pub async fn create_directory(&self, id: TerminalRunId, path: RemotePath) -> Result<()> {
        self.call(|r| Request::Mkdir(id, path, r)).await
    }
    pub async fn rename_path(
        &self,
        id: TerminalRunId,
        source: RemotePath,
        destination: RemotePath,
    ) -> Result<()> {
        self.call(|r| Request::Rename(id, source, destination, r))
            .await
    }
}
impl TerminalSnapshot {
    pub fn is_empty(&self) -> bool {
        self.terminal_runs.is_empty()
            && self.command_runs.is_empty()
            && self.tunnel_runs.is_empty()
            && self.file_operation_runs.is_empty()
    }
    pub fn active_ids(&self) -> Vec<RunId> {
        self.terminal_runs
            .iter()
            .map(|r| RunId::Terminal(r.id))
            .chain(self.command_runs.iter().map(|r| RunId::Command(r.id)))
            .chain(self.tunnel_runs.iter().map(|r| RunId::Tunnel(r.id)))
            .chain(
                self.file_operation_runs
                    .iter()
                    .map(|r| RunId::FileOperation(r.id)),
            )
            .collect()
    }
}
