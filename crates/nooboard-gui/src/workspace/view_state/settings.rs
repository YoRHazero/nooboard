use nooboard_core::WorkspaceSnapshot;

use super::WorkspaceMetricViewState;

#[derive(Clone)]
pub struct SettingsPageViewState {
    pub rows: Vec<WorkspaceMetricViewState>,
}

pub(super) fn build_settings_page_view_state(
    snapshot: &WorkspaceSnapshot,
) -> SettingsPageViewState {
    SettingsPageViewState {
        rows: vec![
            WorkspaceMetricViewState {
                label: "Device ID",
                value: snapshot.settings.connection.device_id.clone(),
            },
            WorkspaceMetricViewState {
                label: "Token",
                value: snapshot.settings.connection.token.clone(),
            },
            WorkspaceMetricViewState {
                label: "Listen Port",
                value: snapshot.settings.network.listen_port.to_string(),
            },
            WorkspaceMetricViewState {
                label: "LAN Enabled",
                value: snapshot.settings.network.lan_enabled.to_string(),
            },
            WorkspaceMetricViewState {
                label: "Local Capture",
                value: snapshot
                    .settings
                    .clipboard
                    .local_capture_enabled
                    .to_string(),
            },
            WorkspaceMetricViewState {
                label: "Download Dir",
                value: snapshot
                    .settings
                    .transfers
                    .download_dir
                    .display()
                    .to_string(),
            },
            WorkspaceMetricViewState {
                label: "History Window",
                value: snapshot.settings.storage.history_window_days.to_string(),
            },
        ],
    }
}
