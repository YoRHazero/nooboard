//! The WebView protocol. Only these types cross the IPC boundary.
use super::errors::UiError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

macro_rules! core_enum {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(Clone, Debug, Serialize, JsonSchema)]
        pub enum $name { $($variant),+ }
        impl From<nooboard_core::$name> for $name {
            fn from(value: nooboard_core::$name) -> Self {
                match value { $(nooboard_core::$name::$variant => Self::$variant),+ }
            }
        }
    };
}
core_enum!(AppState {
    Running,
    Stopping,
    Stopped,
    Failed
});
core_enum!(ClipboardKind {
    Text,
    Image,
    Files,
    Empty,
    Unsupported,
    Sensitive,
    TooLarge
});
core_enum!(ContentKind { Image, Files });
core_enum!(ContentStage {
    Preparing,
    Queued,
    Waiting,
    Sending,
    Receiving,
    Verifying,
    Saving,
    Applying,
    Cancelling,
    Completed,
    Saved,
    Failed,
    Cancelled,
    Unconfirmed
});
core_enum!(TransferFailure {
    Denied,
    Directory,
    Unsupported,
    TooLarge,
    SourceChanged,
    Integrity,
    Io,
    Clipboard,
    Offline,
    Timeout,
    Busy,
    Cancelled,
    Protocol
});
core_enum!(DeliveryState {
    Queued,
    Sending,
    AwaitingReceipt,
    Applied,
    Rejected,
    Unconfirmed,
    Offline,
    Cancelled,
    Superseded,
    QueueFull
});
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
core_enum!(ActivityKind {
    Copied,
    Sent,
    Received
});

#[derive(Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum SendMode {
    Manual,
    Automatic,
}

#[derive(Clone, Serialize, JsonSchema)]
pub struct MessageId {
    pub session: String,
    pub sequence: String,
}
impl From<nooboard_core::MessageId> for MessageId {
    fn from(id: nooboard_core::MessageId) -> Self {
        Self {
            session: id.session().into(),
            sequence: id.sequence().to_string(),
        }
    }
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationStatus {
    pub saved_revision: String,
    pub effective_revision: String,
    pub restart_required: bool,
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
pub struct Peer {
    pub noob_id: String,
    pub device_name: String,
    pub fingerprint: String,
    pub settings: PeerSettings,
    pub online: bool,
    pub accepting: bool,
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
pub struct Clipboard {
    pub revision: String,
    pub kind: ClipboardKind,
    pub text: Option<String>,
    pub source: Option<String>,
    pub copied_at: i64,
    pub files: Vec<String>,
    pub preview: Option<String>,
    pub image_width: Option<u32>,
    pub image_height: Option<u32>,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Delivery {
    pub noob_id: String,
    pub device_name: String,
    pub state: DeliveryState,
}
#[derive(Clone, Serialize, JsonSchema)]
pub struct Transfer {
    pub id: MessageId,
    pub automatic: bool,
    pub bytes: usize,
    pub targets: Vec<Delivery>,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContentTransfer {
    pub key: String,
    pub id: MessageId,
    pub peer: String,
    pub device_name: String,
    pub incoming: bool,
    pub kind: ContentKind,
    pub names: Vec<String>,
    pub total_bytes: u64,
    pub completed_bytes: u64,
    pub stage: ContentStage,
    pub error: Option<TransferFailure>,
    pub saved_paths: Vec<String>,
    pub at: i64,
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
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivityRecord {
    pub sequence: String,
    pub kind: ActivityKind,
    pub summary: String,
    pub at: i64,
    pub source: Option<String>,
    pub device_name: Option<String>,
    pub message_id: Option<MessageId>,
    pub content_task: Option<String>,
    pub content_node: Option<String>,
    pub content_stage: Option<ContentStage>,
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
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub id: String,
    pub text: String,
    pub source: String,
    pub copied_at: i64,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPage {
    pub items: Vec<HistoryItem>,
    pub has_more: bool,
}
#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum HistorySource {
    All,
    Local,
    Remote,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HistoryQuery {
    pub contains: String,
    pub source: HistorySource,
    pub offset: u32,
}
#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ProbeAction {
    Copy,
    Receive,
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
        patch: crate::desktop::Patch,
        #[serde(rename = "legacyLanguage")]
        legacy_language: Option<crate::desktop::Language>,
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
    Host(crate::desktop::Snapshot),
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum Frame {
    Snapshot(BackendSnapshot),
    Recovered(BackendSnapshot),
    Host(crate::desktop::Snapshot),
    Stopped(UiError),
}
#[derive(Serialize, JsonSchema)]
pub struct Connection {
    pub snapshot: BackendSnapshot,
    pub diagnostic: bool,
    pub host: crate::desktop::Snapshot,
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
