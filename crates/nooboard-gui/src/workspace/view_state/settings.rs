use std::path::PathBuf;

use nooboard_core::WorkspaceSnapshot;

#[derive(Clone)]
pub struct SettingsPageViewState {
    pub connection: SettingsConnectionViewState,
    pub clipboard: SettingsClipboardViewState,
    pub transfers: SettingsTransfersViewState,
    pub storage: SettingsStorageViewState,
}

#[derive(Clone)]
pub struct SettingsConnectionViewState {
    pub device_id: String,
    pub token: String,
    pub endpoint_label: Option<String>,
    pub listen_port: u16,
    pub lan_enabled: bool,
}

#[derive(Clone)]
pub struct SettingsClipboardViewState {
    pub local_capture_enabled: bool,
}

#[derive(Clone)]
pub struct SettingsTransfersViewState {
    pub download_dir: PathBuf,
}

#[derive(Clone)]
pub struct SettingsStorageViewState {
    pub db_root: PathBuf,
    pub history_window_days: u32,
    pub dedup_window_days: u32,
    pub max_text_bytes: usize,
    pub gc_batch_size: usize,
}

pub(super) fn build_settings_page_view_state(
    snapshot: &WorkspaceSnapshot,
) -> SettingsPageViewState {
    SettingsPageViewState {
        connection: SettingsConnectionViewState {
            device_id: snapshot.settings.connection.device_id.clone(),
            token: snapshot.settings.connection.token.clone(),
            endpoint_label: snapshot
                .local_connection
                .device_endpoint
                .map(|value| value.to_string()),
            listen_port: snapshot.settings.network.listen_port,
            lan_enabled: snapshot.settings.network.lan_enabled,
        },
        clipboard: SettingsClipboardViewState {
            local_capture_enabled: snapshot.settings.clipboard.local_capture_enabled,
        },
        transfers: SettingsTransfersViewState {
            download_dir: snapshot.settings.transfers.download_dir.clone(),
        },
        storage: SettingsStorageViewState {
            db_root: snapshot.settings.storage.db_root.clone(),
            history_window_days: snapshot.settings.storage.history_window_days,
            dedup_window_days: snapshot.settings.storage.dedup_window_days,
            max_text_bytes: snapshot.settings.storage.max_text_bytes,
            gc_batch_size: snapshot.settings.storage.gc_batch_size,
        },
    }
}
