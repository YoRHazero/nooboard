use tokio::sync::mpsc;

use super::clipboard_history;
use super::command::ControlCommand;
use super::config_patch;
use super::engine_reconcile;
use super::files;
use super::state::ControlState;
use super::subscriptions;

const CONTROL_CHANNEL_CAPACITY: usize = 256;

pub(crate) fn spawn_control_actor(mut state: ControlState) -> mpsc::Sender<ControlCommand> {
    let (command_tx, command_rx) = mpsc::channel(CONTROL_CHANNEL_CAPACITY);

    tokio::spawn(async move {
        run_control_actor(&mut state, command_rx).await;
    });

    command_tx
}

async fn run_control_actor(
    state: &mut ControlState,
    mut command_rx: mpsc::Receiver<ControlCommand>,
) {
    while let Some(command) = command_rx.recv().await {
        if handle_control_command(state, command).await {
            break;
        }
    }

    state
        .subscriptions
        .deactivate(crate::service::types::SubscriptionCloseReason::EngineStopped)
        .await;
    let _ = state.sync_runtime.stop().await;
}

async fn handle_control_command(state: &mut ControlState, command: ControlCommand) -> bool {
    match command {
        ControlCommand::Shutdown { reply } => {
            let result = engine_reconcile::shutdown(state).await;
            let _ = reply.send(result);
            true
        }
        ControlCommand::SetSyncDesiredState {
            desired_state,
            reply,
        } => {
            let _ =
                reply.send(engine_reconcile::set_sync_desired_state(state, desired_state).await);
            false
        }
        ControlCommand::ApplyConfigPatch { patch, reply } => {
            let _ = reply.send(config_patch::apply_config_patch(state, patch).await);
            false
        }
        ControlCommand::Snapshot { reply } => {
            let _ = reply.send(Ok(state.snapshot()));
            false
        }

        ControlCommand::ApplyLocalClipboardChange { request, reply } => {
            let _ =
                reply.send(clipboard_history::apply_local_clipboard_change(state, request).await);
            false
        }
        ControlCommand::ApplyHistoryEntryToClipboard { event_id, reply } => {
            let _ = reply
                .send(clipboard_history::apply_history_entry_to_clipboard(state, event_id).await);
            false
        }
        ControlCommand::ListHistory { request, reply } => {
            let _ = reply.send(clipboard_history::list_history(state, request).await);
            false
        }
        ControlCommand::RebroadcastHistoryEntry { request, reply } => {
            let _ = reply.send(clipboard_history::rebroadcast_history_entry(state, request).await);
            false
        }
        ControlCommand::StoreRemoteText { request, reply } => {
            let _ = reply.send(clipboard_history::store_remote_text(state, request).await);
            false
        }
        ControlCommand::WriteRemoteTextToClipboard { request, reply } => {
            let _ =
                reply.send(clipboard_history::write_remote_text_to_clipboard(state, request).await);
            false
        }

        ControlCommand::SendFile { request, reply } => {
            let _ = reply.send(files::send_file(state, request).await);
            false
        }
        ControlCommand::RespondFileDecision { request, reply } => {
            let _ = reply.send(files::respond_file_decision(state, request).await);
            false
        }

        ControlCommand::SubscribeEvents { reply } => {
            let _ = reply.send(subscriptions::subscribe_events(state).await);
            false
        }
    }
}
