mod bootstrap;
mod defaults;
mod error;
mod io;
mod mapping;
mod noob_id;
mod schema;
mod validate;

pub use bootstrap::{
    BOOTSTRAP_ENV_VAR, BootstrapChooserContext, BootstrapChooserReason, BootstrapDecision,
    BootstrapLaunch, BootstrapMode, BootstrapRequest, ConfigTemplate, CustomLocationProbe,
    DEFAULT_CONFIG_FILE_NAME, ExistingConfigProbe, RepoDevelopmentProbe, default_config_path,
    default_config_root, inspect_custom_location, inspect_existing_config,
    inspect_repo_development, prepare_bootstrap_launch, prepare_custom_location_launch,
    prepare_default_config_from_chooser, prepare_existing_config_launch,
    prepare_repo_development_launch, repo_development_config_path, repo_root_path,
    resolve_bootstrap, resolve_init_output_path, rewrite_existing_config, write_config_template,
};
pub use defaults::{APP_CONFIG_VERSION, DEFAULT_MAX_TEXT_BYTES, DEFAULT_RECENT_EVENT_LOOKUP_LIMIT};
pub use error::{ConfigError, ConfigResult};
pub use schema::{AppConfig, DirectSeedConfig};

#[cfg(test)]
mod tests;
