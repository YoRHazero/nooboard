#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceRoute {
    Home,
    Clipboard,
    Network,
    Transfers,
    Settings,
}

impl Default for WorkspaceRoute {
    fn default() -> Self {
        Self::Home
    }
}
