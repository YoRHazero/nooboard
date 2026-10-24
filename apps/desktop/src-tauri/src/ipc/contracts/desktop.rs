use super::*;
#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ProbeAction {
    Copy,
    Receive,
}

use crate::features::{preferences::domain::Language, window::domain::Navigation};
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub revision: u32,
    pub tray_supported: bool,
    pub tray_available: bool,
    pub close_to_tray: bool,
    pub preference_error: bool,
    pub language: Language,
    pub resolved_language: String,
    pub visible: bool,
    pub navigation: Option<Navigation>,
    pub version: &'static str,
}
