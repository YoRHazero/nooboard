use std::path::{Path, PathBuf};

use nooboard_core::{
    BootstrapChooserContext, BootstrapChooserReason, BootstrapLaunch, CustomLocationProbe,
    ExistingConfigProbe, inspect_custom_location, inspect_existing_config,
    prepare_custom_location_launch, prepare_default_config_from_chooser,
    prepare_existing_config_launch, prepare_repo_development_launch, rewrite_existing_config,
};
use tokio::sync::oneshot;

use super::view_state::BootstrapViewState;

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

pub struct BootstrapController {
    chooser_context: BootstrapChooserContext,
    chooser_state: BootstrapChooserState,
    can_use_repo_development: bool,
    launch_sender: Option<oneshot::Sender<BootstrapLaunch>>,
    launch_in_flight: bool,
    feedback: Option<String>,
}

impl BootstrapController {
    pub fn new(
        context: BootstrapChooserContext,
        can_use_repo_development: bool,
        launch_sender: oneshot::Sender<BootstrapLaunch>,
    ) -> Self {
        Self {
            chooser_context: context,
            chooser_state: BootstrapChooserState::default(),
            can_use_repo_development,
            launch_sender: Some(launch_sender),
            launch_in_flight: false,
            feedback: None,
        }
    }

    pub fn view_state(&self) -> BootstrapViewState {
        let chooser_title = match self.chooser_context.reason {
            BootstrapChooserReason::ExplicitChooserRequest => "Choose Configuration",
            BootstrapChooserReason::MissingDefaultConfig => "Missing Default Config",
            BootstrapChooserReason::DefaultConfigVersionMismatch { .. } => {
                "Update Configuration"
            }
        };

        BootstrapViewState {
            chooser_title,
            selected_preset: self.chooser_state.selected_preset,
            description: self.chooser_state.description(&self.chooser_context),
            feedback: self.feedback.clone(),
            launch_in_flight: self.launch_in_flight,
            can_use_repo_development: self.can_use_repo_development,
            confirm_enabled: self.chooser_state.confirm_enabled(self.can_use_repo_development)
                && !self.launch_in_flight,
            browse_enabled: self.chooser_state.browse_enabled() && !self.launch_in_flight,
            rewrite_visible: self.chooser_state.rewrite_visible(),
            rewrite_enabled: self.chooser_state.rewrite_visible() && !self.launch_in_flight,
        }
    }

    pub fn set_selected_preset(&mut self, preset: BootstrapPreset) {
        self.feedback = None;
        self.chooser_state.select_preset(preset);
    }

    pub fn selected_preset(&self) -> BootstrapPreset {
        self.chooser_state.selected_preset
    }

    pub fn can_use_repo_development(&self) -> bool {
        self.can_use_repo_development
    }

    pub fn begin_action(&mut self) -> bool {
        if self.launch_in_flight || self.launch_sender.is_none() {
            return false;
        }

        self.launch_in_flight = true;
        self.feedback = None;
        true
    }

    pub fn end_action(&mut self) {
        self.launch_in_flight = false;
    }

    pub fn apply_existing_config_selection(&mut self, path: PathBuf) {
        self.launch_in_flight = false;
        self.feedback = None;

        match inspect_existing_config(&path) {
            Ok(probe) => self.chooser_state.set_existing_config_probe(probe),
            Err(error) => {
                self.chooser_state.existing_config = ExistingConfigSelection::Invalid {
                    path,
                    message: error.to_string(),
                };
            }
        }
    }

    pub fn apply_custom_location_selection(&mut self, directory: PathBuf) {
        self.launch_in_flight = false;
        self.feedback = None;

        match inspect_custom_location(&directory) {
            Ok(probe) => self.chooser_state.set_custom_location_probe(probe),
            Err(error) => {
                self.chooser_state.custom_location = CustomLocationSelection::InvalidConfig {
                    config_path: self.chooser_context.default_config_path.clone(),
                    directory,
                    message: error.to_string(),
                };
            }
        }
    }

    pub fn rewrite_existing_config(&mut self) {
        let Some(path) = self
            .chooser_state
            .existing_config
            .path()
            .map(Path::to_path_buf)
        else {
            return;
        };

        if !self.begin_action() {
            return;
        }

        match rewrite_existing_config(&path) {
            Ok(launch) => {
                self.chooser_state.existing_config = ExistingConfigSelection::Valid {
                    path: launch.config_path,
                };
                self.feedback = Some("Configuration file rewritten successfully.".to_string());
                self.end_action();
            }
            Err(error) => {
                self.feedback = Some(error.to_string());
                self.end_action();
            }
        }
    }

    pub fn confirm_selection(&mut self) -> Option<BootstrapLaunch> {
        if !self.begin_action() {
            return None;
        }

        let result = match self.chooser_state.selected_preset {
            BootstrapPreset::DefaultConfig => {
                prepare_default_config_from_chooser(&self.chooser_context).map(Some)
            }
            BootstrapPreset::ExistingConfig => match self.chooser_state.existing_config.valid_path()
            {
                Some(path) => prepare_existing_config_launch(path).map(Some),
                None => Ok(None),
            },
            BootstrapPreset::CustomLocation => match self.chooser_state.custom_location.directory() {
                Some(directory) => prepare_custom_location_launch(directory).map(Some),
                None => Ok(None),
            },
            BootstrapPreset::RepoDevelopment => {
                if self.can_use_repo_development {
                    prepare_repo_development_launch().map(Some)
                } else {
                    Ok(None)
                }
            }
        };

        self.end_action();

        match result {
            Ok(launch) => launch,
            Err(error) => {
                self.feedback = Some(error.to_string());
                None
            }
        }
    }

    pub fn send_launch(&mut self, launch: BootstrapLaunch) {
        if let Some(sender) = self.launch_sender.take() {
            let _ = sender.send(launch);
        }
    }

    pub fn quit(&mut self) {
        self.launch_sender.take();
        self.launch_in_flight = false;
    }
}
