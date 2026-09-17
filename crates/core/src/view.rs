//! Recoverable session state for consumers; no UI framework or persistent activity history.
use crate::{MessageId, Status, Transfer, history::now_ms};
use nooboard_clipboard::{Content, Snapshot};
use serde::Serialize;
use std::collections::VecDeque;

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
            Content::Files(paths) => paths
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
            Content::Text(text) if !text.contains('\0') => (ClipboardKind::Text, Some(text)),
            Content::Empty => (ClipboardKind::Empty, None),
            Content::Sensitive => (ClipboardKind::Sensitive, None),
            Content::TooLarge => (ClipboardKind::TooLarge, None),
            Content::Image(_) => (ClipboardKind::Image, None),
            Content::Files(_) => (ClipboardKind::Files, None),
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
pub(crate) struct ViewState {
    pub snapshot: AppSnapshot,
    activities: VecDeque<ActivityRecord>,
    sequence: u64,
}
impl ViewState {
    pub fn new(session: String, status: Status, initial: Snapshot) -> Self {
        Self {
            snapshot: AppSnapshot {
                content_transfers: Vec::new(),
                local_network: crate::LocalNetwork::default(),
                onboarding: crate::OnboardingSnapshot::default(),
                session,
                revision: 0,
                history_revision: 0,
                status,
                current: CurrentClipboard::from_native(initial, None),
                activities: Vec::new(),
                fault: None,
            },
            activities: VecDeque::new(),
            sequence: 0,
        }
    }
    pub fn current(&mut self, snapshot: Snapshot, source: Option<String>) {
        self.snapshot.current = CurrentClipboard::from_native(snapshot, source);
    }
    pub fn record(
        &mut self,
        kind: ActivityKind,
        text: &str,
        source: Option<String>,
        device_name: Option<String>,
        message_id: Option<MessageId>,
    ) {
        self.sequence += 1;
        let summary = text
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("")
            .trim()
            .chars()
            .take(160)
            .collect();
        self.activities.push_front(ActivityRecord {
            sequence: self.sequence,
            kind,
            summary,
            at_ms: now_ms(),
            source,
            device_name,
            message_id,
            content_task: None,
            content_node: None,
            content_stage: None,
        });
    }
    pub fn content_node(&mut self, task: &crate::ContentTransfer, node: &str) {
        self.record(
            if task.incoming {
                ActivityKind::Received
            } else {
                ActivityKind::Sent
            },
            &task.names.join(", "),
            Some(task.peer.clone()),
            Some(task.device_name.clone()),
            None,
        );
        if let Some(activity) = self.activities.front_mut() {
            activity.content_task = Some(task.key.clone());
            activity.content_node = Some(node.into());
            activity.content_stage = Some(task.stage);
        }
    }
    pub fn publish(&mut self, mut status: Status, transfers: Vec<Transfer>) -> AppSnapshot {
        let mut completed = 0;
        self.activities.retain(|activity| {
            if activity.kind == ActivityKind::Sent
                && transfers
                    .iter()
                    .any(|t| Some(&t.id) == activity.message_id.as_ref() && t.pending())
            {
                return true;
            }
            completed += 1;
            completed <= 30
        });
        status.transfers = transfers;
        self.snapshot.revision += 1;
        self.snapshot.status = status;
        self.snapshot.activities = self.activities.iter().cloned().collect();
        self.snapshot.clone()
    }
}
