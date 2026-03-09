use tokio::sync::{broadcast, mpsc, watch};
use tokio::task::JoinHandle;

use crate::service::mappers::map_sync_connection_error;
use crate::service::types::{EventId, NoobId};

use super::clipboard_history;
use super::command::ControlCommand;
use super::config_patch::{self, reconcile_local_capture_runtime};
use super::engine_reconcile;
use super::files;
use super::state::ControlState;
use super::subscriptions;

const CONTROL_CHANNEL_CAPACITY: usize = 256;

#[derive(Default)]
struct ActorRuntime {
    local_clipboard_task: Option<JoinHandle<()>>,
    sync_event_task: Option<JoinHandle<()>>,
    sync_transfer_task: Option<JoinHandle<()>>,
    sync_status_task: Option<JoinHandle<()>>,
    sync_peers_task: Option<JoinHandle<()>>,
}

pub(crate) fn spawn_control_actor(mut state: ControlState) -> mpsc::Sender<ControlCommand> {
    let (command_tx, command_rx) = mpsc::channel(CONTROL_CHANNEL_CAPACITY);
    let actor_tx = command_tx.clone();

    tokio::spawn(async move {
        let mut runtime = ActorRuntime::default();
        if start_runtime_tasks(&mut runtime, &state, &actor_tx).is_ok() {
            let _ = reconcile_local_capture_runtime(&mut state).await;
        }
        run_control_actor(&mut state, &mut runtime, command_rx).await;
    });

    command_tx
}

async fn run_control_actor(
    state: &mut ControlState,
    runtime: &mut ActorRuntime,
    mut command_rx: mpsc::Receiver<ControlCommand>,
) {
    while let Some(command) = command_rx.recv().await {
        if handle_control_command(state, runtime, command).await {
            break;
        }
    }

    stop_runtime_tasks(runtime).await;
}

async fn handle_control_command(
    state: &mut ControlState,
    _runtime: &mut ActorRuntime,
    command: ControlCommand,
) -> bool {
    match command {
        ControlCommand::Shutdown { reply } => {
            let result = engine_reconcile::shutdown(state).await;
            let _ = reply.send(result);
            true
        }
        ControlCommand::GetState { reply } => {
            let _ = reply.send(Ok(state.get_state()));
            false
        }
        ControlCommand::SubscribeState { reply } => {
            let _ = reply.send(subscriptions::subscribe_state(state));
            false
        }
        ControlCommand::SubscribeEvents { reply } => {
            let _ = reply.send(subscriptions::subscribe_events(state));
            false
        }
        ControlCommand::SetSyncDesiredState {
            desired_state,
            reply,
        } => {
            let _ =
                reply.send(engine_reconcile::set_sync_desired_state(state, desired_state).await);
            false
        }
        ControlCommand::PatchSettings { patch, reply } => {
            let _ = reply.send(config_patch::patch_settings(state, patch).await);
            false
        }
        ControlCommand::SubmitText { request, reply } => {
            let _ = reply.send(clipboard_history::submit_text(state, request).await);
            false
        }
        ControlCommand::GetClipboardRecord { event_id, reply } => {
            let _ = reply.send(clipboard_history::get_clipboard_record(state, event_id).await);
            false
        }
        ControlCommand::ListClipboardHistory { request, reply } => {
            let _ = reply.send(clipboard_history::list_clipboard_history(state, request).await);
            false
        }
        ControlCommand::AdoptClipboardRecord { event_id, reply } => {
            let _ = reply.send(clipboard_history::adopt_clipboard_record(state, event_id).await);
            false
        }
        ControlCommand::RebroadcastClipboardRecord { request, reply } => {
            let _ =
                reply.send(clipboard_history::rebroadcast_clipboard_record(state, request).await);
            false
        }
        ControlCommand::SendFiles { request, reply } => {
            let _ = reply.send(files::send_files(state, request).await);
            false
        }
        ControlCommand::DecideIncomingTransfer { request, reply } => {
            let _ = reply.send(files::decide_incoming_transfer(state, request).await);
            false
        }
        ControlCommand::CancelTransfer { transfer_id, reply } => {
            let _ = reply.send(files::cancel_transfer(state, transfer_id).await);
            false
        }
        ControlCommand::InternalLocalClipboardObserved { observed } => {
            let _ =
                clipboard_history::commit_local_capture(state, observed.event_id, observed.text)
                    .await;
            false
        }
        ControlCommand::InternalSyncEvent { event } => {
            match event.clone() {
                nooboard_sync::SyncEvent::TextReceived {
                    event_id,
                    content,
                    noob_id,
                    device_id,
                } => {
                    if let Ok(parsed_event_id) = EventId::try_from(event_id.as_str()) {
                        let _ = clipboard_history::commit_remote_sync(
                            state,
                            parsed_event_id,
                            content,
                            NoobId::new(noob_id),
                            device_id,
                        )
                        .await;
                    }
                }
                nooboard_sync::SyncEvent::FileDecisionRequired {
                    peer_noob_id,
                    transfer_id,
                    file_name,
                    file_size,
                    total_chunks,
                } => {
                    files::handle_incoming_offer(
                        state,
                        NoobId::new(peer_noob_id),
                        transfer_id,
                        file_name,
                        file_size,
                        total_chunks,
                    );
                }
                nooboard_sync::SyncEvent::ConnectionError { .. } => {
                    if let Some(app_event) = map_sync_connection_error(event) {
                        state.publish_event(app_event);
                    }
                }
            }
            false
        }
        ControlCommand::InternalTransferUpdate { update } => {
            files::apply_transfer_update(state, update);
            false
        }
        ControlCommand::InternalSyncStatusChanged { status } => {
            let actual = status.into();
            state.update_state(|app_state| {
                app_state.sync.actual = actual;
            });
            false
        }
        ControlCommand::InternalConnectedPeersChanged { peers } => {
            let connected = peers
                .into_iter()
                .map(|peer| {
                    let transport = if state.config.sync.network.manual_peers.contains(&peer.addr) {
                        if state.config.sync.network.mdns_enabled {
                            crate::service::types::PeerTransport::Mixed
                        } else {
                            crate::service::types::PeerTransport::Manual
                        }
                    } else if state.config.sync.network.mdns_enabled {
                        crate::service::types::PeerTransport::Mdns
                    } else {
                        crate::service::types::PeerTransport::Unknown
                    };
                    crate::service::mappers::map_connected_peer(peer, transport)
                })
                .collect();
            state.refresh_connected_peers(connected);
            false
        }
    }
}

fn start_runtime_tasks(
    runtime: &mut ActorRuntime,
    state: &ControlState,
    command_tx: &mpsc::Sender<ControlCommand>,
) -> crate::AppResult<()> {
    let mut local_clipboard_rx = state.clipboard.subscribe_local_changes()?;
    let local_tx = command_tx.clone();
    runtime.local_clipboard_task = Some(tokio::spawn(async move {
        loop {
            match local_clipboard_rx.recv().await {
                Ok(observed) => {
                    if local_tx
                        .send(ControlCommand::InternalLocalClipboardObserved { observed })
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
    }));

    let mut sync_event_rx = state.sync_runtime.subscribe_events();
    let event_tx = command_tx.clone();
    runtime.sync_event_task = Some(tokio::spawn(async move {
        loop {
            match sync_event_rx.recv().await {
                Ok(event) => {
                    if event_tx
                        .send(ControlCommand::InternalSyncEvent { event })
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
    }));

    let mut transfer_rx = state.sync_runtime.subscribe_transfer_updates();
    let transfer_tx = command_tx.clone();
    runtime.sync_transfer_task = Some(tokio::spawn(async move {
        loop {
            match transfer_rx.recv().await {
                Ok(update) => {
                    if transfer_tx
                        .send(ControlCommand::InternalTransferUpdate { update })
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
    }));

    let mut status_rx = state.sync_runtime.subscribe_status();
    let status_tx = command_tx.clone();
    runtime.sync_status_task = Some(tokio::spawn(async move {
        forward_watch_changes(
            &mut status_rx,
            move |status| ControlCommand::InternalSyncStatusChanged { status },
            status_tx,
        )
        .await;
    }));

    let mut peers_rx = state.sync_runtime.subscribe_connected_peers();
    let peers_tx = command_tx.clone();
    runtime.sync_peers_task = Some(tokio::spawn(async move {
        forward_watch_changes(
            &mut peers_rx,
            move |peers| ControlCommand::InternalConnectedPeersChanged { peers },
            peers_tx,
        )
        .await;
    }));

    Ok(())
}

async fn forward_watch_changes<T, F>(
    receiver: &mut watch::Receiver<T>,
    mut map_command: F,
    command_tx: mpsc::Sender<ControlCommand>,
) where
    T: Clone + Send + Sync + 'static,
    F: FnMut(T) -> ControlCommand + Send + 'static,
{
    while receiver.changed().await.is_ok() {
        let value = receiver.borrow().clone();
        if command_tx.send(map_command(value)).await.is_err() {
            break;
        }
    }
}

async fn stop_runtime_tasks(runtime: &mut ActorRuntime) {
    abort_task(runtime.local_clipboard_task.take()).await;
    abort_task(runtime.sync_event_task.take()).await;
    abort_task(runtime.sync_transfer_task.take()).await;
    abort_task(runtime.sync_status_task.take()).await;
    abort_task(runtime.sync_peers_task.take()).await;
}

async fn abort_task(task: Option<JoinHandle<()>>) {
    if let Some(task) = task {
        task.abort();
        let _ = task.await;
    }
}
