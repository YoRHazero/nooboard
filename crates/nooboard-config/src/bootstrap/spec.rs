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
pub enum BootstrapChooserReason {
    ExplicitChooserRequest,
    MissingDefaultConfig,
    DefaultConfigVersionMismatch {
        found_version: Option<u32>,
        expected_version: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapChooserContext {
    pub default_config_path: PathBuf,
    pub reason: BootstrapChooserReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootstrapDecision {
    Launch(BootstrapLaunch),
    NeedsChooser(BootstrapChooserContext),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExistingConfigProbe {
    Valid { path: PathBuf },
    Invalid { path: PathBuf, message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomLocationProbe {
    ReadyToCreate {
        directory: PathBuf,
        config_path: PathBuf,
    },
    ExistingValidConfig {
        directory: PathBuf,
        config_path: PathBuf,
    },
    ExistingInvalidConfig {
        directory: PathBuf,
        config_path: PathBuf,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepoDevelopmentProbe {
    Available { config_path: PathBuf },
    Unavailable { message: String },
}
