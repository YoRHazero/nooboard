mod chooser;
mod env;
mod init;
mod launch;
mod paths;
mod probe;
mod reset;
mod resolve;
mod spec;
mod template;

pub use chooser::{
    prepare_custom_location_launch, prepare_existing_config_launch,
    prepare_repo_development_launch, rewrite_existing_config,
};
pub use env::BOOTSTRAP_ENV_VAR;
pub use init::resolve_init_output_path;
pub use launch::prepare_bootstrap_launch;
pub use paths::{
    DEFAULT_CONFIG_FILE_NAME, default_config_path, default_config_root,
    repo_development_config_path, repo_root_path,
};
pub use probe::{inspect_custom_location, inspect_existing_config, inspect_repo_development};
pub use reset::prepare_default_config_from_chooser;
pub use resolve::resolve_bootstrap;
pub use spec::{
    BootstrapChooserContext, BootstrapChooserReason, BootstrapDecision, BootstrapLaunch,
    BootstrapMode, BootstrapRequest, ConfigTemplate, CustomLocationProbe, ExistingConfigProbe,
    RepoDevelopmentProbe,
};
pub use template::write_config_template;
