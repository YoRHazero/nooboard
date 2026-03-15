use std::path::{Path, PathBuf};

use nooboard_core::{BootstrapChooserContext, CustomLocationProbe, ExistingConfigProbe};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapPreset {
    DefaultConfig,
    ExistingConfig,
    CustomLocation,
    RepoDevelopment,
}

impl BootstrapPreset {
    pub fn title(self) -> &'static str {
        match self {
            Self::DefaultConfig => "Use default configuration",
            Self::ExistingConfig => "Use existing config",
            Self::CustomLocation => "Create config in custom location",
            Self::RepoDevelopment => "Use local development setup",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExistingConfigSelection {
    None,
    Valid { path: PathBuf },
    Invalid { path: PathBuf, message: String },
}

impl ExistingConfigSelection {
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::None => None,
            Self::Valid { path } | Self::Invalid { path, .. } => Some(path.as_path()),
        }
    }

    pub fn valid_path(&self) -> Option<&Path> {
        match self {
            Self::Valid { path } => Some(path.as_path()),
            Self::None | Self::Invalid { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomLocationSelection {
    None,
    ReadyToCreate {
        directory: PathBuf,
        config_path: PathBuf,
    },
    ExistingConfig {
        directory: PathBuf,
        config_path: PathBuf,
    },
    InvalidConfig {
        directory: PathBuf,
        config_path: PathBuf,
        message: String,
    },
}

impl CustomLocationSelection {
    pub fn confirm_path(&self) -> Option<&Path> {
        match self {
            Self::ReadyToCreate { config_path, .. } | Self::ExistingConfig { config_path, .. } => {
                Some(config_path.as_path())
            }
            Self::None | Self::InvalidConfig { .. } => None,
        }
    }

    pub fn directory(&self) -> Option<&Path> {
        match self {
            Self::ReadyToCreate { directory, .. }
            | Self::ExistingConfig { directory, .. }
            | Self::InvalidConfig { directory, .. } => Some(directory.as_path()),
            Self::None => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BootstrapChooserState {
    pub selected_preset: BootstrapPreset,
    pub existing_config: ExistingConfigSelection,
    pub custom_location: CustomLocationSelection,
}

impl Default for BootstrapChooserState {
    fn default() -> Self {
        Self {
            selected_preset: BootstrapPreset::DefaultConfig,
            existing_config: ExistingConfigSelection::None,
            custom_location: CustomLocationSelection::None,
        }
    }
}

impl BootstrapChooserState {
    pub fn select_preset(&mut self, preset: BootstrapPreset) {
        self.selected_preset = preset;
    }

    pub fn set_existing_config_probe(&mut self, probe: ExistingConfigProbe) {
        self.existing_config = match probe {
            ExistingConfigProbe::Valid { path } => ExistingConfigSelection::Valid { path },
            ExistingConfigProbe::Invalid { path, message } => {
                ExistingConfigSelection::Invalid { path, message }
            }
        };
    }

    pub fn set_custom_location_probe(&mut self, probe: CustomLocationProbe) {
        self.custom_location = match probe {
            CustomLocationProbe::ReadyToCreate {
                directory,
                config_path,
            } => CustomLocationSelection::ReadyToCreate {
                directory,
                config_path,
            },
            CustomLocationProbe::ExistingValidConfig {
                directory,
                config_path,
            } => CustomLocationSelection::ExistingConfig {
                directory,
                config_path,
            },
            CustomLocationProbe::ExistingInvalidConfig {
                directory,
                config_path,
                message,
            } => CustomLocationSelection::InvalidConfig {
                directory,
                config_path,
                message,
            },
        };
    }

    pub fn browse_enabled(&self) -> bool {
        matches!(
            self.selected_preset,
            BootstrapPreset::ExistingConfig | BootstrapPreset::CustomLocation
        )
    }

    pub fn rewrite_visible(&self) -> bool {
        matches!(
            self.existing_config,
            ExistingConfigSelection::Invalid { .. }
        ) && self.selected_preset == BootstrapPreset::ExistingConfig
    }

    pub fn confirm_enabled(&self, can_use_repo_development: bool) -> bool {
        match self.selected_preset {
            BootstrapPreset::DefaultConfig => true,
            BootstrapPreset::ExistingConfig => self.existing_config.valid_path().is_some(),
            BootstrapPreset::CustomLocation => self.custom_location.confirm_path().is_some(),
            BootstrapPreset::RepoDevelopment => can_use_repo_development,
        }
    }

    pub fn description(&self, chooser: &BootstrapChooserContext) -> String {
        let config_file_name = chooser
            .default_config_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("nooboard.toml");

        match self.selected_preset {
            BootstrapPreset::DefaultConfig => format!(
                "Create a default config and start nooboard.\n{}",
                chooser.default_config_path.display()
            ),
            BootstrapPreset::ExistingConfig => match &self.existing_config {
                ExistingConfigSelection::None => {
                    format!("Choose an existing {config_file_name} file to use for this run.")
                }
                ExistingConfigSelection::Valid { path } => format!(
                    "Selected config will be used for this run.\n{}",
                    path.display()
                ),
                ExistingConfigSelection::Invalid { path, message } => format!(
                    "Selected file is not a valid nooboard config.\n{}\n{}",
                    path.display(),
                    message
                ),
            },
            BootstrapPreset::CustomLocation => match &self.custom_location {
                CustomLocationSelection::None => format!(
                    "Choose a folder. nooboard will create {config_file_name} there if needed."
                ),
                CustomLocationSelection::ReadyToCreate {
                    directory,
                    config_path,
                } => format!(
                    "A new config will be created in the selected folder.\n{}\n{}",
                    directory.display(),
                    config_path.display()
                ),
                CustomLocationSelection::ExistingConfig {
                    directory,
                    config_path,
                } => format!(
                    "The selected folder already contains a valid config and it will be reused.\n{}\n{}",
                    directory.display(),
                    config_path.display()
                ),
                CustomLocationSelection::InvalidConfig {
                    directory,
                    config_path,
                    message,
                } => format!(
                    "The selected folder already contains an invalid {config_file_name}. Choose another folder.\n{}\n{}\n{}",
                    directory.display(),
                    config_path.display(),
                    message
                ),
            },
            BootstrapPreset::RepoDevelopment => {
                "Use the repository-local development setup for this run.".to_string()
            }
        }
    }
}
