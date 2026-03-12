use std::path::{Path, PathBuf};

use nooboard_config::{BootstrapChooserContext, DEFAULT_CONFIG_FILE_NAME};

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

    pub fn set_existing_config_valid(&mut self, path: PathBuf) {
        self.existing_config = ExistingConfigSelection::Valid { path };
    }

    pub fn set_existing_config_invalid(&mut self, path: PathBuf, message: String) {
        self.existing_config = ExistingConfigSelection::Invalid { path, message };
    }

    pub fn set_custom_location_ready_to_create(&mut self, directory: PathBuf) {
        let config_path = directory.join(DEFAULT_CONFIG_FILE_NAME);
        self.custom_location = CustomLocationSelection::ReadyToCreate {
            directory,
            config_path,
        };
    }

    pub fn set_custom_location_existing_config(&mut self, directory: PathBuf) {
        let config_path = directory.join(DEFAULT_CONFIG_FILE_NAME);
        self.custom_location = CustomLocationSelection::ExistingConfig {
            directory,
            config_path,
        };
    }

    pub fn set_custom_location_invalid_config(&mut self, directory: PathBuf, message: String) {
        let config_path = directory.join(DEFAULT_CONFIG_FILE_NAME);
        self.custom_location = CustomLocationSelection::InvalidConfig {
            directory,
            config_path,
            message,
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
        match self.selected_preset {
            BootstrapPreset::DefaultConfig => format!(
                "Create {} and start nooboard with that file.",
                chooser.default_config_path.display()
            ),
            BootstrapPreset::ExistingConfig => match &self.existing_config {
                ExistingConfigSelection::None => {
                    format!(
                        "Choose an existing {DEFAULT_CONFIG_FILE_NAME} file to use for this run."
                    )
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
                    "Choose a folder. nooboard will create {DEFAULT_CONFIG_FILE_NAME} there if needed."
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
                    "The selected folder already contains an invalid {DEFAULT_CONFIG_FILE_NAME}. Choose another folder.\n{}\n{}\n{}",
                    directory.display(),
                    config_path.display(),
                    message
                ),
            },
            BootstrapPreset::RepoDevelopment => {
                "Use the repository-local development setup for this run. A development config will be created in .dev-data if it does not exist.".to_string()
            }
        }
    }
}
