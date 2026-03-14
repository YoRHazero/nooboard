use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::error::CoreResult;

use super::bridges;
use super::command::{WorkspaceBridgeMessage, WorkspaceCommand};
use super::handlers::{clipboard, config, network, storage, transfers};
use super::state::WorkspaceState;

const COMMAND_CHANNEL_CAPACITY: usize = 256;

pub(crate) struct WorkspaceActor {
    pub(crate) command_tx: mpsc::Sender<WorkspaceCommand>,
    pub(crate) task: JoinHandle<()>,
}

#[derive(Default)]
struct ActorRuntime {
    clipboard_bridge: Option<JoinHandle<()>>,
    network_bridge: Option<JoinHandle<()>>,
}

pub(crate) fn spawn_workspace_actor(
    handle: &tokio::runtime::Handle,
    mut state: WorkspaceState,
) -> WorkspaceActor {
    let (command_tx, command_rx) = mpsc::channel(COMMAND_CHANNEL_CAPACITY);
    let actor_tx = command_tx.clone();
    let runtime_handle = handle.clone();
    let task = runtime_handle.spawn(async move {
        let mut runtime = ActorRuntime::default();
        start_bridge_tasks(&mut runtime, &state, &actor_tx);
        run_workspace_actor(&mut state, &mut runtime, actor_tx.clone(), command_rx).await;
    });

    WorkspaceActor { command_tx, task }
}

async fn run_workspace_actor(
    state: &mut WorkspaceState,
    runtime: &mut ActorRuntime,
    actor_tx: mpsc::Sender<WorkspaceCommand>,
    mut command_rx: mpsc::Receiver<WorkspaceCommand>,
) {
    while let Some(command) = command_rx.recv().await {
        if handle_workspace_command(state, runtime, &actor_tx, command).await {
            break;
        }
    }

    stop_bridge_tasks(runtime).await;
}

async fn handle_workspace_command(
    state: &mut WorkspaceState,
    runtime: &mut ActorRuntime,
    actor_tx: &mpsc::Sender<WorkspaceCommand>,
    command: WorkspaceCommand,
) -> bool {
    match command {
        WorkspaceCommand::Shutdown { reply } => {
            let result = shutdown_workspace(state).await;
            let _ = reply.send(result);
            true
        }
        WorkspaceCommand::Snapshot { reply } => {
            let _ = reply.send(Ok(state.snapshot()));
            false
        }
        WorkspaceCommand::SubscribeState { reply } => {
            let _ = reply.send(Ok(state.subscribe_state()));
            false
        }
        WorkspaceCommand::SubscribeEvents { reply } => {
            let _ = reply.send(Ok(state.subscribe_events()));
            false
        }
        WorkspaceCommand::StartNetwork { reply } => {
            let _ = reply.send(network::start_network(state).await);
            false
        }
        WorkspaceCommand::StopNetwork { reply } => {
            let _ = reply.send(network::stop_network(state).await);
            false
        }
        WorkspaceCommand::SetDeviceId { value, reply } => {
            let outcome = config::set_device_id(state, value).await;
            respond_config_outcome(runtime, state, actor_tx, reply, outcome).await;
            false
        }
        WorkspaceCommand::SetNetworkToken { value, reply } => {
            let outcome = config::set_network_token(state, value).await;
            respond_config_outcome(runtime, state, actor_tx, reply, outcome).await;
            false
        }
        WorkspaceCommand::SetNetworkListenPort { value, reply } => {
            let outcome = config::set_network_listen_port(state, value).await;
            respond_config_outcome(runtime, state, actor_tx, reply, outcome).await;
            false
        }
        WorkspaceCommand::SetLanEnabled { value, reply } => {
            let outcome = config::set_lan_enabled(state, value).await;
            respond_config_outcome(runtime, state, actor_tx, reply, outcome).await;
            false
        }
        WorkspaceCommand::SetLocalCaptureEnabled { value, reply } => {
            let outcome = config::set_local_capture_enabled(state, value).await;
            respond_config_outcome(runtime, state, actor_tx, reply, outcome).await;
            false
        }
        WorkspaceCommand::SetDownloadDir { value, reply } => {
            let outcome = config::set_download_dir(state, value).await;
            respond_config_outcome(runtime, state, actor_tx, reply, outcome).await;
            false
        }
        WorkspaceCommand::SetStorageSettings { input, reply } => {
            let outcome = config::set_storage_settings(state, input).await;
            respond_config_outcome(runtime, state, actor_tx, reply, outcome).await;
            false
        }
        WorkspaceCommand::ListDirectSeeds { reply } => {
            let _ = reply.send(network::list_direct_seeds(state).await);
            false
        }
        WorkspaceCommand::UpsertDirectSeed { input, reply } => {
            let outcome = config::upsert_direct_seed(state, input).await;
            respond_config_outcome(runtime, state, actor_tx, reply, outcome).await;
            false
        }
        WorkspaceCommand::RemoveDirectSeed { id, reply } => {
            let outcome = config::remove_direct_seed(state, id).await;
            respond_config_outcome(runtime, state, actor_tx, reply, outcome).await;
            false
        }
        WorkspaceCommand::SearchDirectSeeds { query, reply } => {
            let _ = reply.send(network::search_direct_seeds(state, &query).await);
            false
        }
        WorkspaceCommand::ConnectDirectSeed { id, reply } => {
            let _ = reply.send(network::connect_direct_seed(state, id).await);
            false
        }
        WorkspaceCommand::ListPendingDirectRequests { reply } => {
            let _ = reply.send(network::list_pending_direct_requests(state).await);
            false
        }
        WorkspaceCommand::ApproveDirectRequest { id, reply } => {
            let _ = reply.send(network::approve_direct_request(state, id).await);
            false
        }
        WorkspaceCommand::RejectDirectRequest { id, reply } => {
            let _ = reply.send(network::reject_direct_request(state, id).await);
            false
        }
        WorkspaceCommand::ListSessions { reply } => {
            let _ = reply.send(network::list_sessions(state).await);
            false
        }
        WorkspaceCommand::DisconnectSession { id, reply } => {
            let _ = reply.send(network::disconnect_session(state, id).await);
            false
        }
        WorkspaceCommand::SubmitText { content, reply } => {
            let _ = reply.send(clipboard::submit_text(state, content).await);
            false
        }
        WorkspaceCommand::GetClipboardRecord { event_id, reply } => {
            let _ = reply.send(storage::get_clipboard_record(state, event_id).await);
            false
        }
        WorkspaceCommand::ListClipboardHistory { request, reply } => {
            let _ = reply.send(storage::list_clipboard_history(state, request).await);
            false
        }
        WorkspaceCommand::AdoptClipboardRecord { event_id, reply } => {
            let _ = reply.send(clipboard::adopt_clipboard_record(state, event_id).await);
            false
        }
        WorkspaceCommand::RebroadcastClipboardRecord {
            event_id,
            target,
            reply,
        } => {
            let _ =
                reply.send(clipboard::rebroadcast_clipboard_record(state, event_id, target).await);
            false
        }
        WorkspaceCommand::SendFiles { request, reply } => {
            let _ = reply.send(transfers::send_files(state, request).await);
            false
        }
        WorkspaceCommand::DecideIncomingTransfer { decision, reply } => {
            let _ = reply.send(transfers::decide_incoming_transfer(state, decision).await);
            false
        }
        WorkspaceCommand::CancelTransfer { ticket, reply } => {
            let _ = reply.send(transfers::cancel_transfer(state, ticket).await);
            false
        }
        WorkspaceCommand::Bridge(message) => {
            handle_bridge_message(state, message).await;
            false
        }
    }
}

async fn respond_config_outcome<T>(
    runtime: &mut ActorRuntime,
    state: &WorkspaceState,
    actor_tx: &mpsc::Sender<WorkspaceCommand>,
    reply: oneshot::Sender<CoreResult<T>>,
    outcome: config::ConfigOutcome<T>,
) {
    if outcome.post_action.restart_network_bridge {
        restart_network_bridge(runtime, state, actor_tx).await;
    }
    let _ = reply.send(outcome.result);
}

async fn handle_bridge_message(state: &mut WorkspaceState, message: WorkspaceBridgeMessage) {
    let result = match message {
        WorkspaceBridgeMessage::LocalClipboardObserved(observed) => {
            clipboard::handle_local_clipboard_observed(state, observed).await
        }
        WorkspaceBridgeMessage::NetworkEvent(event) => {
            network::handle_network_event(state, event).await
        }
    };

    if let Err(error) = result {
        tracing::warn!("workspace bridge handling failed: {error}");
    }
}

fn start_bridge_tasks(
    runtime: &mut ActorRuntime,
    state: &WorkspaceState,
    actor_tx: &mpsc::Sender<WorkspaceCommand>,
) {
    runtime.clipboard_bridge = Some(bridges::clipboard::spawn(
        state.clipboard_runtime().subscribe_local_changes(),
        actor_tx.clone(),
    ));
    runtime.network_bridge = Some(bridges::network::spawn(
        state.network_runtime().subscribe(),
        actor_tx.clone(),
    ));
}

async fn restart_network_bridge(
    runtime: &mut ActorRuntime,
    state: &WorkspaceState,
    actor_tx: &mpsc::Sender<WorkspaceCommand>,
) {
    if let Some(handle) = runtime.network_bridge.take() {
        handle.abort();
        let _ = handle.await;
    }
    runtime.network_bridge = Some(bridges::network::spawn(
        state.network_runtime().subscribe(),
        actor_tx.clone(),
    ));
}

async fn stop_bridge_tasks(runtime: &mut ActorRuntime) {
    if let Some(handle) = runtime.clipboard_bridge.take() {
        handle.abort();
        let _ = handle.await;
    }
    if let Some(handle) = runtime.network_bridge.take() {
        handle.abort();
        let _ = handle.await;
    }
}

async fn shutdown_workspace(state: &mut WorkspaceState) -> CoreResult<()> {
    let clipboard_result = state.clipboard_runtime().stop_watch().await;
    let network_result = state.network_runtime().shutdown().await.map_err(Into::into);
    let refresh_result = network::refresh_snapshot_from_runtime(state).await;
    let storage_result = state.storage_runtime().shutdown().await;

    if let Err(error) = clipboard_result {
        return Err(error);
    }
    if let Err(error) = network_result {
        return Err(error);
    }
    if let Err(error) = refresh_result {
        return Err(error);
    }
    storage_result
}
