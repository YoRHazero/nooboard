//! Projection only. Each feature remains the sole owner of its state.
use crate::features::{
    preferences::domain::State as Preferences, tray::domain::State as Tray,
    window::domain::State as Window,
};
#[derive(Clone)]
pub struct Snapshot {
    pub revision: u32,
    pub preferences: Preferences,
    pub window: Window,
    pub tray: Tray,
    pub update: Option<crate::features::update::domain::ReadyUpdate>,
}
