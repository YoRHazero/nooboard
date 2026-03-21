use super::{
    recent_activity::{
        RecentActivityItem, RecentActivityViewState, build_recent_activity_view_state,
    },
    runtime_state::{WorkspaceBridgeState, WorkspaceLoadState},
    view_state::WorkspaceViewState,
};

#[derive(Clone)]
pub struct WorkspaceShellViewState {
    pub headline: String,
    pub subheadline: String,
    pub bootstrap_mode_label: String,
    pub config_path_label: String,
    pub recent_activity: Vec<RecentActivityViewState>,
    pub state_stream_open: bool,
    pub event_stream_open: bool,
    pub bridge_error: Option<String>,
}

pub fn build_workspace_shell_view_state(
    load_state: &WorkspaceLoadState,
    workspace_view: Option<&WorkspaceViewState>,
    recent_activity: &[RecentActivityItem],
    bridge_state: &WorkspaceBridgeState,
    bootstrap_mode_label: String,
    config_path_label: String,
) -> WorkspaceShellViewState {
    let (headline, subheadline) = match (load_state, workspace_view) {
        (WorkspaceLoadState::Failed(message), _) => {
            ("Couldn't open nooboard".to_string(), message.clone())
        }
        (_, Some(workspace_view)) => (
            workspace_view.identity.headline.clone(),
            workspace_view.identity.subheadline.clone(),
        ),
        _ => (
            "Opening nooboard".to_string(),
            "Loading your device and sync status".to_string(),
        ),
    };

    WorkspaceShellViewState {
        headline,
        subheadline,
        bootstrap_mode_label,
        config_path_label,
        recent_activity: build_recent_activity_view_state(recent_activity),
        state_stream_open: bridge_state.state_stream_open,
        event_stream_open: bridge_state.event_stream_open,
        bridge_error: bridge_state.last_error.clone(),
    }
}

#[cfg(test)]
mod tests {
    use crate::workspace::view_state;

    use super::*;

    #[test]
    fn ready_shell_view_uses_workspace_projection() {
        let workspace_view = sample_workspace_view();

        let shell = build_workspace_shell_view_state(
            &WorkspaceLoadState::Ready,
            Some(&workspace_view),
            &[],
            &WorkspaceBridgeState::default(),
            "Default user config".to_string(),
            "/tmp/nooboard.toml".to_string(),
        );

        assert_eq!(shell.headline, "desk-01");
        assert_eq!(shell.subheadline, "local-node");
    }

    #[test]
    fn failed_shell_view_overrides_workspace_labels() {
        let shell = build_workspace_shell_view_state(
            &WorkspaceLoadState::Failed("bad config".to_string()),
            None,
            &[],
            &WorkspaceBridgeState::default(),
            "Explicit path".to_string(),
            "/tmp/nooboard.toml".to_string(),
        );

        assert_eq!(shell.headline, "Couldn't open nooboard");
        assert_eq!(shell.subheadline, "bad config");
    }

    fn sample_workspace_view() -> WorkspaceViewState {
        WorkspaceViewState {
            identity: view_state::WorkspaceIdentityViewState {
                headline: "desk-01".to_string(),
                subheadline: "local-node".to_string(),
            },
            shell_metrics: view_state::WorkspaceShellMetricsViewState {
                session_count_label: "0".to_string(),
                inbox_count_label: "0".to_string(),
            },
            home: view_state::HomePageViewState {
                system_core: view_state::HomeSystemCoreViewState {
                    local_device_id: "desk-01".to_string(),
                    network_control: view_state::HomeNetworkControlViewState {
                        can_start: false,
                        can_stop: true,
                    },
                    clipboard_control: view_state::HomeClipboardControlViewState {
                        adopt_event_id: None,
                    },
                    radar: view_state::HomeRadarViewState {
                        state: view_state::HomeRadarVisualState::Running,
                        peers: Vec::new(),
                    },
                    clipboard: view_state::HomeClipboardPanelViewState {
                        latest_record: None,
                    },
                },
            },
            clipboard: view_state::ClipboardWorkspaceViewState {
                latest_record: None,
                max_text_bytes: 0,
                session_targets: Vec::new(),
            },
            network: view_state::NetworkPageViewState {
                status_label: "Running".to_string(),
                network_enabled: true,
                local_device_id: "desk-01".to_string(),
                local_noob_id: "local-node".to_string(),
                network_token: "shared-token".to_string(),
                endpoint_label: "127.0.0.1:17890".to_string(),
                lan_enabled: true,
                direct_seed_count: 0,
                pending_request_count: 0,
                direct_session_count: 0,
                connected_lan_peer_count: 0,
                lan_peers: Vec::new(),
                direct_seeds: Vec::new(),
                pending_requests: Vec::new(),
                direct_sessions: Vec::new(),
            },
            transfers: view_state::TransfersPageViewState {
                available_targets: Vec::new(),
                incoming: Vec::new(),
                active: Vec::new(),
                completed: Vec::new(),
            },
            settings: view_state::SettingsPageViewState {
                connection: view_state::SettingsConnectionViewState {
                    device_id: "desk-01".to_string(),
                    token: "shared-token".to_string(),
                    endpoint_label: Some("127.0.0.1:17890".to_string()),
                    listen_port: 17890,
                    lan_enabled: true,
                },
                clipboard: view_state::SettingsClipboardViewState {
                    local_capture_enabled: true,
                },
                transfers: view_state::SettingsTransfersViewState {
                    download_dir: "/tmp/downloads".into(),
                },
                storage: view_state::SettingsStorageViewState {
                    db_root: "/tmp/db".into(),
                    history_window_days: 7,
                    dedup_window_days: 14,
                    max_text_bytes: 4096,
                    gc_batch_size: 64,
                },
            },
        }
    }
}
