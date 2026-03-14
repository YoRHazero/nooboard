pub use nooboard_config::{
    BootstrapChooserContext, BootstrapChooserReason, BootstrapDecision, BootstrapLaunch,
    BootstrapMode, BootstrapRequest,
};

use crate::error::CoreResult;

pub fn resolve_bootstrap(request: &BootstrapRequest) -> CoreResult<BootstrapDecision> {
    nooboard_config::resolve_bootstrap(request).map_err(Into::into)
}

pub fn prepare_default_config_from_chooser(
    context: &BootstrapChooserContext,
) -> CoreResult<BootstrapLaunch> {
    nooboard_config::prepare_default_config_from_chooser(context).map_err(Into::into)
}

pub(crate) fn prepare_bootstrap_launch(launch: &BootstrapLaunch) -> CoreResult<()> {
    nooboard_config::prepare_bootstrap_launch(launch).map_err(Into::into)
}
