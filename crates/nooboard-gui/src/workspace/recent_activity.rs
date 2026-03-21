use std::time::{SystemTime, UNIX_EPOCH};

use nooboard_core::{
    ClipboardRecordSource, ConnectionFailure, EventId, NetworkStatus, TransferOutcome,
    TransferTicket, WorkspaceEvent,
};
use time::{OffsetDateTime, UtcOffset};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecentActivitySeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecentActivityKind {
    ClipboardCommitted {
        event_id: EventId,
        source: ClipboardRecordSource,
    },
    IncomingTransferOffered {
        ticket: TransferTicket,
    },
    TransferCompleted {
        ticket: TransferTicket,
        outcome: TransferOutcome,
    },
    NetworkConnectionFailed {
        failure: ConnectionFailure,
    },
    NetworkStarting,
    NetworkRunning,
    NetworkStopped,
    NetworkError {
        message: String,
    },
    GuiWarning {
        message: String,
    },
    GuiError {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentActivityItem {
    pub observed_at_ms: i64,
    pub severity: RecentActivitySeverity,
    pub kind: RecentActivityKind,
}

impl RecentActivityItem {
    pub fn new(kind: RecentActivityKind) -> Self {
        Self {
            observed_at_ms: now_millis(),
            severity: activity_severity(&kind),
            kind,
        }
    }
}

#[derive(Clone)]
pub struct RecentActivityViewState {
    pub label: &'static str,
    pub title: String,
    pub time_label: String,
    pub severity: RecentActivitySeverity,
}

pub fn recent_activity_from_workspace_event(event: &WorkspaceEvent) -> Option<RecentActivityItem> {
    let kind = match event {
        WorkspaceEvent::ClipboardCommitted { event_id, source } => {
            RecentActivityKind::ClipboardCommitted {
                event_id: *event_id,
                source: *source,
            }
        }
        WorkspaceEvent::IncomingTransferOffered { ticket } => {
            RecentActivityKind::IncomingTransferOffered { ticket: *ticket }
        }
        WorkspaceEvent::TransferUpdated { .. } => return None,
        WorkspaceEvent::TransferCompleted { ticket, outcome } => {
            RecentActivityKind::TransferCompleted {
                ticket: *ticket,
                outcome: *outcome,
            }
        }
        WorkspaceEvent::NetworkConnectionFailed { failure } => {
            RecentActivityKind::NetworkConnectionFailed {
                failure: failure.clone(),
            }
        }
    };

    Some(RecentActivityItem::new(kind))
}

pub fn recent_activity_from_network_status(status: &NetworkStatus) -> RecentActivityItem {
    let kind = match status {
        NetworkStatus::Starting => RecentActivityKind::NetworkStarting,
        NetworkStatus::Running => RecentActivityKind::NetworkRunning,
        NetworkStatus::Stopped => RecentActivityKind::NetworkStopped,
        NetworkStatus::Error(message) => RecentActivityKind::NetworkError {
            message: message.clone(),
        },
    };

    RecentActivityItem::new(kind)
}

pub fn build_recent_activity_view_state(
    activity: &[RecentActivityItem],
) -> Vec<RecentActivityViewState> {
    activity
        .iter()
        .take(5)
        .map(|item| RecentActivityViewState {
            label: activity_kind_label(item),
            title: activity_title(item),
            time_label: clock_label_from_millis(item.observed_at_ms),
            severity: item.severity,
        })
        .collect()
}

fn activity_severity(kind: &RecentActivityKind) -> RecentActivitySeverity {
    match kind {
        RecentActivityKind::ClipboardCommitted { .. }
        | RecentActivityKind::IncomingTransferOffered { .. }
        | RecentActivityKind::TransferCompleted { .. }
        | RecentActivityKind::NetworkStarting
        | RecentActivityKind::NetworkRunning
        | RecentActivityKind::NetworkStopped => RecentActivitySeverity::Info,
        RecentActivityKind::NetworkConnectionFailed { .. }
        | RecentActivityKind::GuiWarning { .. } => RecentActivitySeverity::Warning,
        RecentActivityKind::NetworkError { .. } | RecentActivityKind::GuiError { .. } => {
            RecentActivitySeverity::Error
        }
    }
}

fn activity_kind_label(item: &RecentActivityItem) -> &'static str {
    match item.kind {
        RecentActivityKind::ClipboardCommitted { .. } => "Clipboard",
        RecentActivityKind::IncomingTransferOffered { .. } => "Incoming File",
        RecentActivityKind::TransferCompleted { .. } => "Transfer Complete",
        RecentActivityKind::NetworkConnectionFailed { .. } => "Connection Problem",
        RecentActivityKind::NetworkStarting => "Network",
        RecentActivityKind::NetworkRunning => "Network",
        RecentActivityKind::NetworkStopped => "Network",
        RecentActivityKind::NetworkError { .. } => "Network Problem",
        RecentActivityKind::GuiWarning { .. } => "App Warning",
        RecentActivityKind::GuiError { .. } => "App Error",
    }
}

fn activity_title(item: &RecentActivityItem) -> String {
    match &item.kind {
        RecentActivityKind::ClipboardCommitted { source, .. } => {
            format!("Saved a clipboard item from {}", clipboard_source_label(*source))
        }
        RecentActivityKind::IncomingTransferOffered { ticket } => {
            format!("A file is waiting for your approval ({ticket:?})")
        }
        RecentActivityKind::TransferCompleted { ticket, outcome } => {
            format!("A transfer finished ({ticket:?}, {}).", transfer_outcome_label(*outcome))
        }
        RecentActivityKind::NetworkConnectionFailed { failure } => failure.detail.clone(),
        RecentActivityKind::NetworkStarting => "Starting network sharing.".to_string(),
        RecentActivityKind::NetworkRunning => "Network sharing is on.".to_string(),
        RecentActivityKind::NetworkStopped => "Network sharing is off.".to_string(),
        RecentActivityKind::NetworkError { message } => message.clone(),
        RecentActivityKind::GuiWarning { message } | RecentActivityKind::GuiError { message } => {
            message.clone()
        }
    }
}

fn clipboard_source_label(source: ClipboardRecordSource) -> &'static str {
    match source {
        ClipboardRecordSource::LocalCapture => "this device",
        ClipboardRecordSource::RemoteSync => "another device",
        ClipboardRecordSource::UserSubmit => "manual save",
    }
}

fn transfer_outcome_label(outcome: TransferOutcome) -> &'static str {
    match outcome {
        TransferOutcome::Succeeded => "completed",
        TransferOutcome::Rejected => "rejected",
        TransferOutcome::Cancelled => "cancelled",
        TransferOutcome::Failed => "failed",
    }
}

fn clock_label_from_millis(timestamp_ms: i64) -> String {
    let offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    let nanos = i128::from(timestamp_ms) * 1_000_000;
    let datetime = OffsetDateTime::from_unix_timestamp_nanos(nanos)
        .unwrap_or(OffsetDateTime::UNIX_EPOCH)
        .to_offset(offset);

    format!(
        "{:02}:{:02}:{:02}",
        datetime.hour(),
        datetime.minute(),
        datetime.second()
    )
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use nooboard_core::EventId;

    use super::*;

    #[test]
    fn build_recent_activity_view_state_keeps_five_newest_rows() {
        let items = (0..6)
            .map(|index| RecentActivityItem {
                observed_at_ms: index,
                severity: RecentActivitySeverity::Warning,
                kind: RecentActivityKind::GuiWarning {
                    message: format!("warning-{index}"),
                },
            })
            .collect::<Vec<_>>();

        let view = build_recent_activity_view_state(&items);

        assert_eq!(view.len(), 5);
        assert_eq!(view[0].label, "App Warning");
        assert_eq!(view[0].title, "warning-0");
    }

    #[test]
    fn workspace_event_projection_preserves_clipboard_metadata() {
        let event = WorkspaceEvent::ClipboardCommitted {
            event_id: EventId::new(),
            source: ClipboardRecordSource::UserSubmit,
        };

        let item = recent_activity_from_workspace_event(&event).expect("event should project");

        assert!(matches!(
            item.kind,
            RecentActivityKind::ClipboardCommitted {
                source: ClipboardRecordSource::UserSubmit,
                ..
            }
        ));
    }
}
