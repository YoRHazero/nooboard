mod actions;
mod components;
mod view;

use nooboard_core::{BootstrapChooserContext, BootstrapLaunch};
use tokio::sync::oneshot;

use crate::bootstrap::BootstrapController;

pub struct BootstrapChooserView {
    controller: BootstrapController,
}

impl BootstrapChooserView {
    pub fn new(
        context: BootstrapChooserContext,
        can_use_repo_development: bool,
        launch_sender: oneshot::Sender<BootstrapLaunch>,
    ) -> Self {
        Self {
            controller: BootstrapController::new(context, can_use_repo_development, launch_sender),
        }
    }
}
