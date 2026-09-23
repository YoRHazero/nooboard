use crate::history::now_ms;
use crate::{MessageId, PeerSettings, Settings};
use nooboard_clipboard::{Payload, ReadState, SkipReason, Snapshot};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum AppState {
    Running,
    Stopping,
    Stopped,
    Failed,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PeerStatus {
    pub noob_id: String,
    pub device_name: String,
    pub fingerprint: String,
    pub settings: PeerSettings,
    pub online: bool,
    pub accepting: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Status {
    pub state: AppState,
    pub restart_required: bool,
    pub configuration_revision: u64,
    pub effective_revision: u64,
    pub noob_id: String,
    pub fingerprint: String,
    /// Actual bound address, including the selected port when configured with port 0.
    pub listen_address: String,
    pub settings: Settings,
    pub peers: Vec<PeerStatus>,
    pub manual_targets: Vec<String>,
    pub transfers: Vec<crate::Transfer>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Event {
    Transfer(crate::Transfer),
    HistoryChanged,
}

#[derive(Clone, Copy, Serialize, PartialEq, Eq)]
pub enum ClipboardKind {
    Text,
    Image,
    Files,
    Empty,
    Unsupported,
    Sensitive,
    TooLarge,
}
#[derive(Clone, Serialize)]
pub struct CurrentClipboard {
    pub revision: u64,
    pub kind: ClipboardKind,
    pub text: Option<String>,
    pub source: Option<String>,
    pub copied_at_ms: i64,
    pub files: Vec<String>,
    pub preview: Option<String>,
    pub image_width: Option<u32>,
    pub image_height: Option<u32>,
}
impl CurrentClipboard {
    pub(crate) fn from_native(snapshot: Snapshot, source: Option<String>) -> Self {
        let files = match &snapshot.content {
            ReadState::Ready(Payload::Files(paths)) => paths
                .iter()
                .map(|p| {
                    p.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                })
                .collect(),
            _ => Vec::new(),
        };
        let (kind, text) = match snapshot.content {
            ReadState::Ready(Payload::Text(text)) if !text.contains('\0') => {
                (ClipboardKind::Text, Some(text))
            }
            ReadState::Empty => (ClipboardKind::Empty, None),
            ReadState::Skipped(SkipReason::Sensitive) => (ClipboardKind::Sensitive, None),
            ReadState::Skipped(SkipReason::TooLarge) => (ClipboardKind::TooLarge, None),
            ReadState::Ready(Payload::Image(_)) => (ClipboardKind::Image, None),
            ReadState::Ready(Payload::Files(_)) => (ClipboardKind::Files, None),
            _ => (ClipboardKind::Unsupported, None),
        };
        Self {
            revision: snapshot.revision,
            kind,
            text,
            source,
            copied_at_ms: now_ms(),
            files,
            preview: None,
            image_width: None,
            image_height: None,
        }
    }
}
#[derive(Clone, Copy, Serialize, PartialEq, Eq)]
pub enum ActivityKind {
    Copied,
    Sent,
    Received,
}
#[derive(Clone, Serialize)]
pub struct ActivityRecord {
    pub sequence: u64,
    pub kind: ActivityKind,
    pub summary: String,
    pub at_ms: i64,
    pub source: Option<String>,
    pub device_name: Option<String>,
    pub message_id: Option<MessageId>,
    pub content_task: Option<String>,
    pub content_node: Option<String>,
    pub content_stage: Option<crate::ContentStage>,
}
#[derive(Clone, Serialize)]
pub struct Fault {
    pub sequence: u64,
    pub peer: Option<String>,
    pub message: String,
}
#[derive(Clone, Serialize)]
pub struct AppSnapshot {
    pub content_transfers: Vec<crate::ContentTransfer>,
    pub local_network: crate::LocalNetwork,
    pub onboarding: crate::OnboardingSnapshot,
    pub session: String,
    pub revision: u64,
    pub history_revision: u64,
    pub status: Status,
    pub current: CurrentClipboard,
    pub activities: Vec<ActivityRecord>,
    pub fault: Option<Fault>,
}
