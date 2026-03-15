use nooboard_core::SessionId;

#[derive(Clone)]
pub struct WorkspaceSessionTargetViewState {
    pub id: SessionId,
    pub device_id: String,
    pub mode_label: String,
    pub remote_addr_label: String,
}
