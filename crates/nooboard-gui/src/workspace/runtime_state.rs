#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceBridgeState {
    pub state_stream_open: bool,
    pub event_stream_open: bool,
    pub last_error: Option<String>,
}

impl Default for WorkspaceBridgeState {
    fn default() -> Self {
        Self {
            state_stream_open: true,
            event_stream_open: true,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceLoadState {
    Loading,
    Ready,
    Failed(String),
}
