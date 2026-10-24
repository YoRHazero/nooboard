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
pub enum Action {
    Show(Option<Page>),
    Quit,
}
#[derive(Clone, Copy)]
pub struct State {
    pub supported: bool,
    pub available: bool,
}
pub struct Presentation {
    pub status: String,
    pub open: String,
    pub transfers: String,
    pub settings: String,
    pub quit: String,
}
