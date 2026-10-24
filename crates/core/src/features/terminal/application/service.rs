use super::{api::TerminalWorkspace, requests::Request};
use crate::{
    Error, Result,
    contracts::terminal::CancellationToken,
    features::terminal::{domain::views::TerminalIssue, ports::engine::TerminalEngineOwner},
};
use tokio::task::JoinHandle;
pub(crate) struct TerminalFeatureService {
    pub workspace: TerminalWorkspace,
    pub owner: Box<dyn TerminalEngineOwner>,
    pub task: Option<JoinHandle<()>>,
    pub stop: CancellationToken,
}
impl TerminalFeatureService {
    pub async fn shutdown(&mut self) -> Result<()> {
        if self.task.is_none() {
            return Ok(());
        }
        // A failed coordinator must not prevent native cleanup.
        let _ = self.workspace.coordinator.call(Request::Closing).await;
        if let Err(error) = self.owner.shutdown().await {
            self.workspace
                .coordinator
                .issue(TerminalIssue::Service {
                    message: error.to_string(),
                })
                .await;
            return Err(error);
        }
        let _ = self.workspace.coordinator.0.send(Request::Finish).await;
        let result = match self.task.as_mut() {
            Some(task) => task.await.map_err(|_| Error::Internal),
            None => Ok(()),
        };
        self.task.take();
        result
    }
    /// Drop cleanup continues to retain its dependencies while native resources settle.
    pub async fn drain(&mut self) {
        let mut snapshots = self.workspace.engine.subscribe().snapshots;
        loop {
            if self.shutdown().await.is_ok() {
                return;
            }
            // No polling or deadline that abandons ownership. A terminal manager
            // failure requires retaining dependencies until process termination.
            if snapshots.next().await.is_none() {
                std::future::pending::<()>().await;
            }
        }
    }
}
impl Drop for TerminalFeatureService {
    fn drop(&mut self) {
        self.stop.cancel();
    }
}
