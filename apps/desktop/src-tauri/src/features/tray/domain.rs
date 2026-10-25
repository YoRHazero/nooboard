use crate::features::window::domain::Page;
#[derive(Default, Clone, PartialEq, Eq)]
pub enum Summary {
    #[default]
    Starting,
    Running {
        paused: bool,
        connected: usize,
        transfers: usize,
    },
    Failed,
}
#[derive(Clone, Copy)]
#[cfg_attr(
    not(any(target_os = "macos", target_os = "windows")),
    allow(dead_code, reason = "unsupported platforms cannot emit tray actions")
)]
pub enum Action {
    Show(Option<Page>),
    Quit,
}
#[derive(Clone, Copy)]
pub struct State {
    pub supported: bool,
    pub available: bool,
}
#[cfg_attr(
    not(any(target_os = "macos", target_os = "windows")),
    allow(
        dead_code,
        reason = "unsupported platforms do not render the tray menu"
    )
)]
pub struct Presentation {
    pub status: String,
    pub open: String,
    pub transfers: String,
    pub settings: String,
    pub quit: String,
}
