pub use nooboard_config::{
    BootstrapChooserContext, BootstrapChooserReason, BootstrapDecision, BootstrapLaunch,
    BootstrapMode, BootstrapRequest, CustomLocationProbe, ExistingConfigProbe,
    RepoDevelopmentProbe,
};

use crate::error::CoreResult;

pub fn resolve_bootstrap(request: &BootstrapRequest) -> CoreResult<BootstrapDecision> {
    nooboard_config::resolve_bootstrap(request).map_err(Into::into)
}

pub fn inspect_existing_config(path: &std::path::Path) -> CoreResult<ExistingConfigProbe> {
    nooboard_config::inspect_existing_config(path).map_err(Into::into)
}

pub fn inspect_custom_location(directory: &std::path::Path) -> CoreResult<CustomLocationProbe> {
    nooboard_config::inspect_custom_location(directory).map_err(Into::into)
}

pub fn inspect_repo_development() -> CoreResult<RepoDevelopmentProbe> {
    nooboard_config::inspect_repo_development().map_err(Into::into)
}

pub fn prepare_default_config_from_chooser(
    context: &BootstrapChooserContext,
) -> CoreResult<BootstrapLaunch> {
    nooboard_config::prepare_default_config_from_chooser(context).map_err(Into::into)
}

pub fn prepare_existing_config_launch(path: &std::path::Path) -> CoreResult<BootstrapLaunch> {
    nooboard_config::prepare_existing_config_launch(path).map_err(Into::into)
}

pub fn rewrite_existing_config(path: &std::path::Path) -> CoreResult<BootstrapLaunch> {
    nooboard_config::rewrite_existing_config(path).map_err(Into::into)
}

pub fn prepare_custom_location_launch(directory: &std::path::Path) -> CoreResult<BootstrapLaunch> {
    nooboard_config::prepare_custom_location_launch(directory).map_err(Into::into)
}

pub fn prepare_repo_development_launch() -> CoreResult<BootstrapLaunch> {
    nooboard_config::prepare_repo_development_launch().map_err(Into::into)
}

pub(crate) fn prepare_bootstrap_launch(launch: &BootstrapLaunch) -> CoreResult<()> {
    nooboard_config::prepare_bootstrap_launch(launch).map_err(Into::into)
}
