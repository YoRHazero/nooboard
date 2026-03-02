use super::super::WorkspaceView;

impl WorkspaceView {
    pub(crate) fn sync_label(&self) -> String {
        format!("{:?}", self.state.app.sync_status)
    }

    pub(crate) fn desired_state_label(&self) -> String {
        format!("{:?}", self.state.app.desired_state)
    }
}
