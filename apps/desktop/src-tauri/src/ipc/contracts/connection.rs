use super::*;
core_enum!(AppState {
    Running,
    Stopping,
    Stopped,
    Failed
});
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationStatus {
    pub saved_revision: String,
    pub effective_revision: String,
    pub restart_required: bool,
}
#[derive(Clone, Serialize, JsonSchema)]
pub struct Fault {
    pub sequence: String,
    pub message: UiError,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BackendSnapshot {
    pub session: String,
    pub revision: String,
    pub history_revision: String,
    pub state: AppState,
    pub configuration: ConfigurationStatus,
    pub settings: SyncSettings,
    pub local_device: LocalDevice,
    pub current: Clipboard,
    pub peers: Vec<Peer>,
    pub manual_targets: Vec<String>,
    pub transfers: Vec<Transfer>,
    pub content_transfers: Vec<ContentTransfer>,
    pub onboarding: Onboarding,
    pub activities: Vec<ActivityRecord>,
    pub fault: Option<Fault>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(
    tag = "type",
    content = "data",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub enum Request {
    Discover,
    BeginPairing {
        address: String,
        expected: Option<String>,
    },
    AcceptPairing {
        id: String,
    },
    PairingCode {
        id: String,
        code: String,
    },
    DismissPairing {
        id: String,
    },
    UpdateSyncSettings {
        patch: SyncSettingsPatch,
    },
    UpdateLocalDevice {
        patch: LocalDevicePatch,
    },
    ConfigurePeer {
        #[serde(rename = "noobId")]
        noob_id: String,
        patch: PeerPatch,
    },
    SelectTargets {
        targets: Vec<String>,
    },
    Unpair {
        #[serde(rename = "noobId")]
        noob_id: String,
    },
    SendCurrent,
    SelectFiles,
    SelectReceiveDirectory,
    CancelTransfer {
        key: String,
    },
    CopyReceived {
        key: String,
    },
    QueryHistory {
        query: HistoryQuery,
    },
    CopyHistory {
        id: String,
    },
    DeleteHistory {
        id: String,
    },
    ClearHistory,
    HostPreferences {
        patch: crate::features::preferences::domain::Patch,
        #[serde(rename = "legacyLanguage")]
        legacy_language: Option<crate::features::preferences::domain::Language>,
    },
    AcknowledgeNavigation {
        id: u32,
    },
    Probe {
        action: ProbeAction,
    },
}
#[derive(Serialize, JsonSchema)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum Reply {
    Done,
    Sent(MessageId),
    Saved { revision: String },
    History(HistoryPage),
    Host(Snapshot),
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum Frame {
    Snapshot(BackendSnapshot),
    Recovered(BackendSnapshot),
    Host(Snapshot),
    Stopped(UiError),
}
#[derive(Serialize, JsonSchema)]
pub struct Connection {
    pub snapshot: BackendSnapshot,
    pub diagnostic: bool,
    pub host: Snapshot,
}
/// Schema root includes both directions. It is never instantiated or sent.
#[derive(JsonSchema)]
#[allow(dead_code)]
pub struct Contract {
    connection: Connection,
    frame: Frame,
    request: Request,
    reply: Reply,
}
