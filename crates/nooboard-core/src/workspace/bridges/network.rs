use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use nooboard_network::{NetworkEventRecvError, NetworkSubscription};

use super::super::command::{WorkspaceBridgeMessage, WorkspaceCommand};

pub(crate) fn spawn(
    mut subscription: NetworkSubscription,
    command_tx: mpsc::Sender<WorkspaceCommand>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            match subscription.recv().await {
                Ok(event) => {
                    if command_tx
                        .send(WorkspaceCommand::Bridge(
                            WorkspaceBridgeMessage::NetworkEvent(event),
                        ))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(NetworkEventRecvError::Closed) => break,
            }
        }
    })
}
