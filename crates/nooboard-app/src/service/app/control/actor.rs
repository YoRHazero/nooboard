use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;

use crate::service::types::{IngestTextRequest, NoobId, TextSource};

use super::clipboard_history;
use super::command::ControlCommand;
use super::config_patch;
use super::engine_reconcile;
use super::files;
use super::state::ControlState;
use super::subscriptions;

const CONTROL_CHANNEL_CAPACITY: usize = 256;

#[derive(Default)]
struct ActorRuntime {
    local_watch_enabled: bool,
    local_watch_task: Option<JoinHandle<()>>,
    sync_ingest_task: Option<JoinHandle<()>>,
}

pub(crate) fn spawn_control_actor(mut state: ControlState) -> mpsc::Sender<ControlCommand> {
    let (command_tx, command_rx) = mpsc::channel(CONTROL_CHANNEL_CAPACITY);
    let actor_tx = command_tx.clone();

    tokio::spawn(async move {
        run_control_actor(&mut state, command_rx, actor_tx).await;
    });

    command_tx
}

async fn run_control_actor(
    state: &mut ControlState,
    mut command_rx: mpsc::Receiver<ControlCommand>,
    command_tx: mpsc::Sender<ControlCommand>,
) {
    let mut runtime = ActorRuntime::default();
    while let Some(command) = command_rx.recv().await {
        if handle_control_command(state, &mut runtime, &command_tx, command).await {
            break;
        }
    }

    stop_runtime_tasks(&mut runtime).await;
    state
        .subscriptions
        .deactivate(crate::service::types::SubscriptionCloseReason::EngineStopped)
        .await;
    let _ = state.clipboard.stop_watch().await;
    let _ = state.sync_runtime.stop().await;
}

async fn handle_control_command(
    state: &mut ControlState,
    runtime: &mut ActorRuntime,
    command_tx: &mpsc::Sender<ControlCommand>,
    command: ControlCommand,
) -> bool {
    match command {
        ControlCommand::Shutdown { reply } => {
            stop_runtime_tasks(runtime).await;
            let result = engine_reconcile::shutdown(state).await;
            let _ = reply.send(result);
            true
        }
        ControlCommand::SetSyncDesiredState {
            desired_state,
            reply,
        } => {
            let result = engine_reconcile::set_sync_desired_state(state, desired_state).await;
            let _ = reconcile_sync_ingest_bridge(state, runtime, command_tx).await;
            let _ = reply.send(result);
            false
        }
        ControlCommand::ApplyConfigPatch { patch, reply } => {
            let result = config_patch::apply_config_patch(state, patch).await;
            let _ = reconcile_sync_ingest_bridge(state, runtime, command_tx).await;
            let _ = reply.send(result);
            false
        }
        ControlCommand::Snapshot { reply } => {
            let _ = reply.send(Ok(state.snapshot()));
            false
        }

        ControlCommand::IngestTextEvent { request, reply } => {
            let _ = reply.send(clipboard_history::ingest_text_event(state, request).await);
            false
        }
        ControlCommand::WriteEventToClipboard { event_id, reply } => {
            let _ = reply.send(clipboard_history::write_event_to_clipboard(state, event_id).await);
            false
        }
        ControlCommand::ListHistory { request, reply } => {
            let _ = reply.send(clipboard_history::list_history(state, request).await);
            false
        }
        ControlCommand::RebroadcastEvent { request, reply } => {
            let _ = reply.send(clipboard_history::rebroadcast_event(state, request).await);
            false
        }
        ControlCommand::SetLocalWatchEnabled { enabled, reply } => {
            runtime.local_watch_enabled = enabled;
            let result = reconcile_local_watch_bridge(state, runtime, command_tx).await;
            let _ = reply.send(result);
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
        ControlCommand::SubscribeLocalClipboard { reply } => {
            let _ = reply.send(subscriptions::subscribe_local_clipboard(state));
            false
        }
        ControlCommand::InternalLocalClipboardObserved { observed } => {
            let request = IngestTextRequest {
                event_id: observed.event_id,
                content: observed.text,
                origin_noob_id: NoobId::new(state.config.noob_id().unwrap_or_default().to_string()),
                origin_device_id: state.config.identity.device_id.clone(),
                source: TextSource::LocalWatch,
            };
            let _ = clipboard_history::ingest_text_event(state, request).await;
            false
        }
        ControlCommand::InternalSyncEvent { event } => {
            if let nooboard_sync::SyncEvent::TextReceived {
                event_id,
                content,
                noob_id,
                device_id,
            } = event
            {
                match crate::service::types::EventId::try_from(event_id.as_str()) {
                    Ok(parsed_event_id) => {
                        let request = IngestTextRequest {
                            event_id: parsed_event_id,
                            content,
                            origin_noob_id: NoobId::new(noob_id),
                            origin_device_id: device_id,
                            source: TextSource::RemoteSync,
                        };
                        let _ = clipboard_history::ingest_text_event(state, request).await;
                    }
                    Err(_) => {}
                }
            }
            false
        }
    }
}

async fn reconcile_local_watch_bridge(
    state: &mut ControlState,
    runtime: &mut ActorRuntime,
    command_tx: &mpsc::Sender<ControlCommand>,
) -> crate::AppResult<()> {
    if runtime.local_watch_enabled {
        state.clipboard.start_watch()?;
        if runtime.local_watch_task.is_none() {
            let mut subscription = state.clipboard.subscribe_local_changes()?;
            let bridge_tx = command_tx.clone();
            runtime.local_watch_task = Some(tokio::spawn(async move {
                loop {
                    match subscription.recv().await {
                        Ok(observed) => {
                            if bridge_tx
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
        }
        return Ok(());
    }

    abort_task(runtime.local_watch_task.take()).await;
    state.clipboard.stop_watch().await
}

async fn reconcile_sync_ingest_bridge(
    state: &mut ControlState,
    runtime: &mut ActorRuntime,
    command_tx: &mpsc::Sender<ControlCommand>,
) -> crate::AppResult<()> {
    abort_task(runtime.sync_ingest_task.take()).await;

    if !state.sync_runtime.has_engine() {
        return Ok(());
    }

    let mut sync_rx = match state.sync_runtime.subscribe_events() {
        Ok(rx) => rx,
        Err(_) => return Ok(()),
    };

    let bridge_tx = command_tx.clone();
    runtime.sync_ingest_task = Some(tokio::spawn(async move {
        loop {
            match sync_rx.recv().await {
                Ok(event) => {
                    if bridge_tx
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

    Ok(())
}

async fn stop_runtime_tasks(runtime: &mut ActorRuntime) {
    abort_task(runtime.local_watch_task.take()).await;
    abort_task(runtime.sync_ingest_task.take()).await;
}

async fn abort_task(task: Option<JoinHandle<()>>) {
    if let Some(task) = task {
        task.abort();
        let _ = task.await;
    }
}
