use std::path::PathBuf;

use gpui::{Context, Entity, Task};
use nooboard_core::StorageSettingsInput;

use crate::workspace::controller::WorkspaceController;

use super::spawn::spawn_core_call;

pub fn set_device_id_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    value: String,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<()>>> {
    spawn_core_call(
        controller,
        cx,
        "failed to set device id",
        move |core| async move { core.set_device_id(value).await },
    )
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
    spawn_core_call(
        controller,
        cx,
        "failed to set LAN enabled",
        move |core| async move { core.set_lan_enabled(value).await },
    )
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
