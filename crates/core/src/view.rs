//! Recoverable session state for consumers; no UI framework or persistent activity history.
use crate::{MessageId, Status, Transfer, history::now_ms};
use nooboard_clipboard::{Content, Snapshot};
use serde::Serialize;
use std::collections::VecDeque;

#[derive(Clone, Copy, Serialize, PartialEq, Eq)]
pub enum ClipboardKind {
    Text,
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
}
impl CurrentClipboard {
    pub(crate) fn from_native(snapshot: Snapshot, source: Option<String>) -> Self {
        let (kind, text) = match snapshot.content {
            Content::Text(text) if !text.contains('\0') => (ClipboardKind::Text, Some(text)),
            Content::Empty => (ClipboardKind::Empty, None),
            Content::Sensitive => (ClipboardKind::Sensitive, None),
            Content::TooLarge => (ClipboardKind::TooLarge, None),
            _ => (ClipboardKind::Unsupported, None),
        };
        Self {
            revision: snapshot.revision,
            kind,
            text,
            source,
            copied_at_ms: now_ms(),
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
}
#[derive(Clone, Serialize)]
pub struct Fault {
    pub sequence: u64,
    pub peer: Option<String>,
    pub message: String,
}
#[derive(Clone, Serialize)]
pub struct AppSnapshot {
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
        });
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
