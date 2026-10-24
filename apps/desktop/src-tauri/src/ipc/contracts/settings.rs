use super::*;
#[derive(Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum SendMode {
    Manual,
    Automatic,
}

#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncSettings {
    pub receive_directory: Option<String>,
    pub discoverable: bool,
    pub mode: SendMode,
    pub receive: bool,
    pub paused: bool,
    pub history: bool,
    pub max_history_entries: u32,
    pub history_days: u32,
}
#[derive(Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyncSettingsPatch {
    pub discoverable: Option<bool>,
    pub mode: Option<SendMode>,
    pub receive: Option<bool>,
    pub paused: Option<bool>,
    pub history: Option<bool>,
    pub max_history_entries: Option<u32>,
    pub history_days: Option<u32>,
}
#[derive(Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalDevicePatch {
    pub device_name: Option<String>,
    pub pairing_port: Option<std::num::NonZeroU16>,
}
