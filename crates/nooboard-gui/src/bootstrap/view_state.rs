use super::controller::BootstrapPreset;

#[derive(Clone)]
pub struct BootstrapViewState {
    pub chooser_title: &'static str,
    pub selected_preset: BootstrapPreset,
    pub description: String,
    pub feedback: Option<String>,
    pub launch_in_flight: bool,
    pub can_use_repo_development: bool,
    pub confirm_enabled: bool,
    pub browse_enabled: bool,
    pub rewrite_visible: bool,
    pub rewrite_enabled: bool,
}
