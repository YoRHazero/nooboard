#[allow(dead_code)]
pub mod actions;
pub mod controller;
pub mod core_bridge;
pub mod recent_activity;
pub mod runtime_state;
pub mod shell_view_state;
pub mod subscriptions;
pub mod view_state;

use nooboard_core::BootstrapLaunch;

#[derive(Clone)]
pub enum LaunchHandle {
    Ready(BootstrapLaunch),
}
