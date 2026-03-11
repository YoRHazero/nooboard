mod env;
mod paths;
mod resolve;
mod spec;
mod template;

pub use env::BOOTSTRAP_ENV_VAR;
pub use paths::{
    DEFAULT_CONFIG_FILE_NAME, default_config_path, default_config_root,
    repo_development_config_path,
};
pub use resolve::resolve_bootstrap;
pub use spec::{
    BootstrapChooserContext, BootstrapDecision, BootstrapLaunch, BootstrapMode, BootstrapRequest,
    ConfigTemplate,
};
pub use template::write_config_template;
