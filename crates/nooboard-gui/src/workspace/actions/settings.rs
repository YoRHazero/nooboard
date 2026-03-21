use std::path::PathBuf;

use gpui::{Context, Entity, Task};
use nooboard_core::StorageSettingsInput;

use crate::workspace::controller::WorkspaceController;

use super::spawn::spawn_core_call;

#[derive(Clone)]
pub struct ApplyConnectionSettingsInput {
    pub device_id: String,
    pub token: String,
    pub listen_port: u16,
    pub lan_enabled: bool,
    pub current_device_id: String,
    pub current_token: String,
    pub current_listen_port: u16,
    pub current_lan_enabled: bool,
}

#[derive(Clone)]
pub struct ApplyClipboardSettingsInput {
    pub local_capture_enabled: bool,
    pub current_local_capture_enabled: bool,
}

pub fn set_device_id_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    value: String,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<()>>> {
    spawn_core_call(
        controller,
        cx,
        "couldn't save the device name",
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
        "couldn't save the network token",
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
        "couldn't save the connection port",
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
        "couldn't update nearby discovery",
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
        "couldn't update local clipboard sharing",
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
        "couldn't save the download folder",
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
        "couldn't save storage settings",
        move |core| async move { core.set_storage_settings(input).await },
    )
}

pub fn apply_connection_settings_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    input: ApplyConnectionSettingsInput,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<()>>> {
    spawn_core_call(
        controller,
        cx,
        "couldn't save connection settings",
        move |core| async move {
            if input.device_id != input.current_device_id {
                core.set_device_id(input.device_id.clone()).await?;
            }
            if input.token != input.current_token {
                core.set_network_token(input.token.clone()).await?;
            }
            if input.listen_port != input.current_listen_port {
                core.set_network_listen_port(input.listen_port).await?;
            }
            if input.lan_enabled != input.current_lan_enabled {
                core.set_lan_enabled(input.lan_enabled).await?;
            }
            Ok(())
        },
    )
}

pub fn apply_clipboard_settings_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    input: ApplyClipboardSettingsInput,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<()>>> {
    spawn_core_call(
        controller,
        cx,
        "couldn't save clipboard settings",
        move |core| async move {
            if input.local_capture_enabled != input.current_local_capture_enabled {
                core.set_local_capture_enabled(input.local_capture_enabled)
                    .await?;
            }
            Ok(())
        },
    )
}
