use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;

use crate::clipboard::LocalClipboardSubscription;

use super::super::command::{WorkspaceBridgeMessage, WorkspaceCommand};

pub(crate) fn spawn(
    mut subscription: LocalClipboardSubscription,
    command_tx: mpsc::Sender<WorkspaceCommand>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            match subscription.recv().await {
                Ok(observed) => {
                    if command_tx
                        .send(WorkspaceCommand::Bridge(
                            WorkspaceBridgeMessage::LocalClipboardObserved(observed),
                        ))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    })
}
