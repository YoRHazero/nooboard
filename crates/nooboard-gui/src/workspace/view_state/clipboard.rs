use nooboard_core::{ClipboardRecord, WorkspaceSnapshot};

use super::WorkspaceSessionTargetViewState;

#[derive(Clone)]
pub struct ClipboardWorkspaceViewState {
    pub latest_record: Option<ClipboardRecord>,
    pub max_text_bytes: usize,
    pub session_targets: Vec<WorkspaceSessionTargetViewState>,
}

pub(super) fn build_clipboard_workspace_view_state(
    snapshot: &WorkspaceSnapshot,
    latest_record: Option<&ClipboardRecord>,
) -> ClipboardWorkspaceViewState {
    ClipboardWorkspaceViewState {
        latest_record: latest_record.cloned(),
        max_text_bytes: snapshot.settings.storage.max_text_bytes,
        session_targets: snapshot
            .network
            .sessions
            .iter()
            .map(|session| WorkspaceSessionTargetViewState {
                id: session.id,
                device_id: session.peer_device_id.clone(),
                mode_label: format!("{:?}", session.mode),
                remote_addr_label: session.remote_addr.to_string(),
            })
            .collect(),
    }
}
