#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceRoute {
    Home,
    Clipboard,
    History,
    Peers,
    Transfers,
    Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuickPanelTab {
    Send,
    Inbox,
    Recent,
}
