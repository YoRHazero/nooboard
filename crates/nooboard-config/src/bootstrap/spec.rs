use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct BootstrapRequest {
    pub cli_choose_config: bool,
    pub cli_config_path: Option<PathBuf>,
    pub cli_use_repo_dev: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapMode {
    ExplicitPath,
    RepoDevelopment,
    UserDefault,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigTemplate {
    Production,
    Development,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapLaunch {
    pub mode: BootstrapMode,
    pub config_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapChooserContext {
    pub default_config_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootstrapDecision {
    Launch(BootstrapLaunch),
    NeedsChooser(BootstrapChooserContext),
}
