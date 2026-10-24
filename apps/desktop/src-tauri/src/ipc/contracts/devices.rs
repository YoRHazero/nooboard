use super::*;
core_enum!(PairingStage {
    Requesting,
    AwaitingApproval,
    ShowingCode,
    EnteringCode,
    Verifying,
    Saving,
    Completed,
    Failed
});
#[derive(Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PeerSettings {
    pub address: Option<String>,
    pub auto_send: bool,
}
#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum AddressChange {
    Clear,
    Set(String),
}
#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PeerPatch {
    pub address: Option<AddressChange>,
    pub auto_send: Option<bool>,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum PeerConnectionState {
    Connected,
    Paused,
    Offline,
    Unconnected,
    Unpaired,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Peer {
    pub noob_id: String,
    pub device_name: String,
    pub fingerprint: String,
    pub settings: PeerSettings,
    pub online: bool,
    pub connection_address: Option<String>,
    pub accepting: bool,
    pub connection_state: PeerConnectionState,
    pub paused: bool,
    pub unpaired_notice: Option<String>,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LocalAddress {
    pub interface: String,
    pub ip: String,
    pub pairing_address: String,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LocalDevice {
    pub noob_id: String,
    pub device_name: String,
    pub fingerprint: String,
    pub platform: String,
    pub sync_port: u16,
    pub pairing_port: u16,
    pub addresses: Vec<LocalAddress>,
    pub address_error: Option<UiError>,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NearbyDevice {
    pub key: String,
    pub noob_id: String,
    pub device_name: String,
    pub addresses: Vec<String>,
    pub sync_port: u16,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PairingSession {
    pub id: String,
    pub incoming: bool,
    pub device_name: String,
    pub noob_id: Option<String>,
    pub stage: PairingStage,
    pub code: Option<String>,
    pub expires_at: i64,
    pub attempts_left: u8,
    pub error: Option<UiError>,
}
#[derive(Clone, Serialize, JsonSchema)]
pub struct Onboarding {
    pub nearby: Vec<NearbyDevice>,
    pub error: Option<UiError>,
    pub session: Option<PairingSession>,
}
