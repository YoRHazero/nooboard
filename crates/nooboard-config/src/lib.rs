mod bootstrap;
mod defaults;
mod error;
mod io;
mod mapping;
mod noob_id;
mod schema;
mod validate;

pub use bootstrap::{
    BOOTSTRAP_ENV_VAR, BootstrapChooserContext, BootstrapDecision, BootstrapLaunch, BootstrapMode,
    BootstrapRequest, ConfigTemplate, DEFAULT_CONFIG_FILE_NAME, default_config_path,
    default_config_root, prepare_bootstrap_launch, repo_development_config_path, repo_root_path,
    resolve_bootstrap, resolve_init_output_path, write_config_template,
};
pub use defaults::{APP_CONFIG_VERSION, DEFAULT_MAX_TEXT_BYTES, DEFAULT_RECENT_EVENT_LOOKUP_LIMIT};
pub use error::{ConfigError, ConfigResult};
pub use schema::AppConfig;

#[cfg(test)]
mod tests;
