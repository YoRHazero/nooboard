use std::path::{Path, PathBuf};

use nooboard_core::{
    BootstrapChooserContext, BootstrapChooserReason, BootstrapLaunch, inspect_custom_location,
    inspect_existing_config, prepare_custom_location_launch, prepare_default_config_from_chooser,
    prepare_existing_config_launch, prepare_repo_development_launch, rewrite_existing_config,
};
use tokio::sync::oneshot;

use super::{
    state::{
        BootstrapChooserState, BootstrapPreset, CustomLocationSelection, ExistingConfigSelection,
    },
    view_state::BootstrapViewState,
};

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
            BootstrapPreset::ExistingConfig => match self.chooser_state.existing_config.valid_path() {
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
