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
    pub revision_label: String,
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
    config_path_label: String,
) -> WorkspaceShellViewState {
    let (headline, subheadline, revision_label) = match (load_state, workspace_view) {
        (WorkspaceLoadState::Failed(message), _) => (
            "Workspace launch failed".to_string(),
            message.clone(),
            "launch failed".to_string(),
        ),
        (_, Some(workspace_view)) => (
            workspace_view.identity.headline.clone(),
            workspace_view.identity.subheadline.clone(),
            workspace_view.identity.revision_label.clone(),
        ),
        _ => (
            "Launching workspace".to_string(),
            "Waiting for nooboard-core snapshot".to_string(),
            "initializing".to_string(),
        ),
    };

    WorkspaceShellViewState {
        headline,
        subheadline,
        revision_label,
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
            "/tmp/nooboard.toml".to_string(),
        );

        assert_eq!(shell.headline, "desk-01");
        assert_eq!(shell.subheadline, "local-node");
        assert_eq!(shell.revision_label, "revision 7");
    }

    #[test]
    fn failed_shell_view_overrides_workspace_labels() {
        let shell = build_workspace_shell_view_state(
            &WorkspaceLoadState::Failed("bad config".to_string()),
            None,
            &[],
            &WorkspaceBridgeState::default(),
            "/tmp/nooboard.toml".to_string(),
        );

        assert_eq!(shell.headline, "Workspace launch failed");
        assert_eq!(shell.subheadline, "bad config");
        assert_eq!(shell.revision_label, "launch failed");
    }

    fn sample_workspace_view() -> WorkspaceViewState {
        WorkspaceViewState {
            identity: view_state::WorkspaceIdentityViewState {
                headline: "desk-01".to_string(),
                subheadline: "local-node".to_string(),
                revision_label: "revision 7".to_string(),
            },
            shell_metrics: view_state::WorkspaceShellMetricsViewState {
                session_count_label: "0".to_string(),
                transfer_count_label: "0".to_string(),
            },
            home: view_state::HomePageViewState {
                metrics: Vec::new(),
                latest_clipboard_preview: String::new(),
                latest_clipboard_event_id: None,
                latest_clipboard_source: None,
            },
            clipboard: view_state::ClipboardWorkspaceViewState {
                latest_record: None,
                max_text_bytes: 0,
                session_targets: Vec::new(),
            },
            network: view_state::NetworkPageViewState {
                status_label: "Running".to_string(),
                can_start: false,
                can_stop: true,
                lan_peers: Vec::new(),
                direct_seeds: Vec::new(),
                pending_requests: Vec::new(),
                sessions: Vec::new(),
            },
            transfers: view_state::TransfersPageViewState {
                incoming: Vec::new(),
                active: Vec::new(),
                completed: Vec::new(),
            },
            settings: view_state::SettingsPageViewState { rows: Vec::new() },
        }
    }
}
