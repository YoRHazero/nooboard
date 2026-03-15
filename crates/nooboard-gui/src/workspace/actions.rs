use std::{future::Future, path::PathBuf};

use gpui::{Context, Entity, Task};
use nooboard_core::{
    DirectRequestId, DirectSeedId, EventId, IncomingTransferDecision, NooboardCore,
    SendFilesRequest, SessionId, SessionTarget, StorageSettingsInput, TransferTicket,
    UpsertDirectSeedInput,
};

use super::controller::WorkspaceController;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceRoute {
    Home,
    Clipboard,
    Network,
    Transfers,
    Settings,
}

impl Default for WorkspaceRoute {
    fn default() -> Self {
        Self::Home
    }
}

fn spawn_core_call<T: 'static, R: 'static, Fut>(
    controller: &Entity<WorkspaceController>,
    cx: &Context<T>,
    error_prefix: &'static str,
    op: impl FnOnce(std::sync::Arc<NooboardCore>) -> Fut + 'static,
) -> Option<Task<nooboard_core::CoreResult<R>>>
where
    Fut: Future<Output = nooboard_core::CoreResult<R>> + 'static,
{
    let Some(core) = controller.read(cx).core() else {
        return None;
    };
    let controller = controller.downgrade();

    Some(cx.spawn(async move |_, cx| {
        let result = op(core).await;
        if let Err(error) = &result {
            let _ = controller.update(cx, |this, cx| {
                this.record_bridge_warning(format!("{error_prefix}: {error}"));
                cx.notify();
            });
        }
        result
    }))
}

pub fn start_network<T: 'static>(controller: &Entity<WorkspaceController>, cx: &Context<T>) {
    if let Some(task) = start_network_task(controller, cx) {
        task.detach();
    }
}

pub fn start_network_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<()>>> {
    spawn_core_call(controller, cx, "failed to start network", |core| async move {
        core.start_network().await
    })
}

pub fn stop_network<T: 'static>(controller: &Entity<WorkspaceController>, cx: &Context<T>) {
    if let Some(task) = stop_network_task(controller, cx) {
        task.detach();
    }
}

pub fn stop_network_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<()>>> {
    spawn_core_call(controller, cx, "failed to stop network", |core| async move {
        core.stop_network().await
    })
}

pub fn set_device_id_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    value: String,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<()>>> {
    spawn_core_call(controller, cx, "failed to set device id", move |core| async move {
        core.set_device_id(value).await
    })
}

pub fn set_network_token_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    value: String,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<()>>> {
    spawn_core_call(
        controller,
        cx,
        "failed to set network token",
        move |core| async move { core.set_network_token(value).await },
    )
}

pub fn set_network_listen_port_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    value: u16,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<()>>> {
    spawn_core_call(
        controller,
        cx,
        "failed to set listen port",
        move |core| async move { core.set_network_listen_port(value).await },
    )
}

pub fn set_lan_enabled_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    value: bool,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<()>>> {
    spawn_core_call(controller, cx, "failed to set LAN enabled", move |core| async move {
        core.set_lan_enabled(value).await
    })
}

pub fn set_local_capture_enabled_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    value: bool,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<()>>> {
    spawn_core_call(
        controller,
        cx,
        "failed to set local capture",
        move |core| async move { core.set_local_capture_enabled(value).await },
    )
}

pub fn set_download_dir_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    value: PathBuf,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<()>>> {
    spawn_core_call(
        controller,
        cx,
        "failed to set download directory",
        move |core| async move { core.set_download_dir(value).await },
    )
}

pub fn set_storage_settings_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    input: StorageSettingsInput,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<()>>> {
    spawn_core_call(
        controller,
        cx,
        "failed to set storage settings",
        move |core| async move { core.set_storage_settings(input).await },
    )
}

pub fn upsert_direct_seed_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    input: UpsertDirectSeedInput,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<DirectSeedId>>> {
    spawn_core_call(
        controller,
        cx,
        "failed to upsert direct seed",
        move |core| async move { core.upsert_direct_seed(input).await },
    )
}

pub fn remove_direct_seed_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    id: DirectSeedId,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<()>>> {
    spawn_core_call(
        controller,
        cx,
        "failed to remove direct seed",
        move |core| async move { core.remove_direct_seed(id).await },
    )
}

pub fn connect_direct_seed_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    id: DirectSeedId,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<nooboard_core::ConnectDirectOutcome>>> {
    spawn_core_call(
        controller,
        cx,
        "failed to connect direct seed",
        move |core| async move { core.connect_direct_seed(id).await },
    )
}

pub fn approve_direct_request_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    id: DirectRequestId,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<()>>> {
    spawn_core_call(
        controller,
        cx,
        "failed to approve direct request",
        move |core| async move { core.approve_direct_request(id).await },
    )
}

pub fn reject_direct_request_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    id: DirectRequestId,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<()>>> {
    spawn_core_call(
        controller,
        cx,
        "failed to reject direct request",
        move |core| async move { core.reject_direct_request(id).await },
    )
}

pub fn disconnect_session_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    id: SessionId,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<()>>> {
    spawn_core_call(
        controller,
        cx,
        "failed to disconnect session",
        move |core| async move { core.disconnect_session(id).await },
    )
}

pub fn submit_text_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    content: String,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<EventId>>> {
    spawn_core_call(
        controller,
        cx,
        "failed to submit clipboard text",
        move |core| async move { core.submit_text(content).await },
    )
}

pub fn adopt_clipboard_record_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    event_id: EventId,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<()>>> {
    spawn_core_call(
        controller,
        cx,
        "failed to adopt clipboard record",
        move |core| async move { core.adopt_clipboard_record(event_id).await },
    )
}

pub fn rebroadcast_clipboard_record_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    event_id: EventId,
    target: SessionTarget,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<()>>> {
    spawn_core_call(
        controller,
        cx,
        "failed to rebroadcast clipboard record",
        move |core| async move { core.rebroadcast_clipboard_record(event_id, target).await },
    )
}

pub fn send_files_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    request: SendFilesRequest,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<Vec<TransferTicket>>>> {
    spawn_core_call(
        controller,
        cx,
        "failed to send files",
        move |core| async move { core.send_files(request).await },
    )
}

pub fn decide_incoming_transfer_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    decision: IncomingTransferDecision,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<()>>> {
    spawn_core_call(
        controller,
        cx,
        "failed to decide incoming transfer",
        move |core| async move { core.decide_incoming_transfer(decision).await },
    )
}

pub fn cancel_transfer_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    ticket: TransferTicket,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<()>>> {
    spawn_core_call(
        controller,
        cx,
        "failed to cancel transfer",
        move |core| async move { core.cancel_transfer(ticket).await },
    )
}

pub fn adopt_latest_clipboard<T: 'static>(
    controller: &Entity<WorkspaceController>,
    cx: &Context<T>,
) {
    let event_id = controller
        .read(cx)
        .latest_committed_record()
        .map(|record| record.event_id);
    let Some(event_id) = event_id else {
        return;
    };

    let Some(core) = controller.read(cx).core() else {
        return;
    };
    let controller = controller.downgrade();

    cx.spawn(async move |_, cx| {
        if let Err(error) = core.adopt_clipboard_record(event_id).await {
            let _ = controller.update(cx, |this, cx| {
                this.record_clipboard_adopt_failed(event_id, error.to_string());
                cx.notify();
            });
        }
        Ok::<_, anyhow::Error>(())
    })
    .detach();
}
