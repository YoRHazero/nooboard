use gpui::Hsla;
use gpui_component::IconName;

use crate::state::live_app::{RecentActivityItem, RecentActivityKind, RecentActivitySeverity};
use crate::ui::theme;

use super::clock_label_from_millis;

pub(crate) fn activity_kind_icon(item: &RecentActivityItem) -> IconName {
    match item.kind {
        RecentActivityKind::ClipboardCommitted { .. } => IconName::Copy,
        RecentActivityKind::ClipboardAdoptFailed { .. } => IconName::TriangleAlert,
        RecentActivityKind::IncomingTransferOffered { .. }
        | RecentActivityKind::TransferCompleted { .. } => IconName::Folder,
        RecentActivityKind::PeerConnectionError { .. }
        | RecentActivityKind::SyncDisabledBySettings
        | RecentActivityKind::SyncError { .. }
        | RecentActivityKind::DesktopWarning { .. }
        | RecentActivityKind::DesktopError { .. } => IconName::TriangleAlert,
        RecentActivityKind::SyncStarting
        | RecentActivityKind::SyncRunning
        | RecentActivityKind::SyncStopped => IconName::Globe,
    }
}

pub(crate) fn activity_accent(item: &RecentActivityItem) -> Hsla {
    match item.severity {
        RecentActivitySeverity::Info => match item.kind {
            RecentActivityKind::TransferCompleted { .. }
            | RecentActivityKind::IncomingTransferOffered { .. } => theme::accent_amber(),
            RecentActivityKind::SyncStarting
            | RecentActivityKind::SyncRunning
            | RecentActivityKind::SyncStopped => theme::accent_cyan(),
            RecentActivityKind::ClipboardCommitted { .. } => theme::accent_blue(),
            _ => theme::accent_cyan(),
        },
        RecentActivitySeverity::Warning => theme::accent_amber(),
        RecentActivitySeverity::Error => theme::accent_rose(),
    }
}

pub(crate) fn activity_kind_label(item: &RecentActivityItem) -> &'static str {
    match item.kind {
        RecentActivityKind::ClipboardCommitted { .. } => "Clipboard",
        RecentActivityKind::ClipboardAdoptFailed { .. } => "Clipboard Warning",
        RecentActivityKind::IncomingTransferOffered { .. } => "Incoming Transfer",
        RecentActivityKind::TransferCompleted { .. } => "Transfer Complete",
        RecentActivityKind::PeerConnectionError { .. } => "Peer Error",
        RecentActivityKind::SyncStarting => "Sync Starting",
        RecentActivityKind::SyncRunning => "Sync Running",
        RecentActivityKind::SyncStopped => "Sync Stopped",
        RecentActivityKind::SyncDisabledBySettings => "Sync Disabled",
        RecentActivityKind::SyncError { .. } => "Sync Error",
        RecentActivityKind::DesktopWarning { .. } => "Desktop Warning",
        RecentActivityKind::DesktopError { .. } => "Desktop Error",
    }
}

pub(crate) fn activity_title(item: &RecentActivityItem) -> String {
    match &item.kind {
        RecentActivityKind::ClipboardCommitted { source, .. } => {
            format!("clipboard record committed from {:?}", source)
        }
        RecentActivityKind::ClipboardAdoptFailed { event_id, message } => {
            format!("clipboard record {event_id} was saved, but local adopt failed: {message}")
        }
        RecentActivityKind::IncomingTransferOffered { transfer_id } => {
            format!("incoming transfer {transfer_id} is awaiting a decision")
        }
        RecentActivityKind::TransferCompleted {
            transfer_id,
            outcome,
        } => format!("transfer {transfer_id} completed with {:?}", outcome),
        RecentActivityKind::PeerConnectionError {
            peer_noob_id,
            addr,
            error,
        } => match (peer_noob_id, addr) {
            (Some(noob_id), Some(addr)) => {
                format!("connection to peer {noob_id} at {addr} failed: {error}")
            }
            (Some(noob_id), None) => format!("connection to peer {noob_id} failed: {error}"),
            (None, Some(addr)) => format!("connection to {addr} failed: {error}"),
            (None, None) => format!("connection failed: {error}"),
        },
        RecentActivityKind::SyncStarting => "sync runtime is starting".to_string(),
        RecentActivityKind::SyncRunning => "sync runtime is running".to_string(),
        RecentActivityKind::SyncStopped => "sync runtime is stopped".to_string(),
        RecentActivityKind::SyncDisabledBySettings => {
            "sync runtime is disabled by network settings".to_string()
        }
        RecentActivityKind::SyncError { message } => message.clone(),
        RecentActivityKind::DesktopWarning { message }
        | RecentActivityKind::DesktopError { message } => message.clone(),
    }
}

pub(crate) fn activity_time_label(item: &RecentActivityItem) -> String {
    clock_label_from_millis(item.observed_at_ms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::live_app::{RecentActivityItem, RecentActivityKind, RecentActivitySeverity};

    #[test]
    fn peer_error_title_describes_local_connection_failure() {
        let item = RecentActivityItem {
            observed_at_ms: 0,
            severity: RecentActivitySeverity::Warning,
            kind: RecentActivityKind::PeerConnectionError {
                peer_noob_id: None,
                addr: Some("100.64.5.26:17890".parse().expect("valid socket addr")),
                error: "I/O error: can't assign requested address (os error 49)".to_string(),
            },
        };

        assert_eq!(
            activity_title(&item),
            "connection to 100.64.5.26:17890 failed: I/O error: can't assign requested address (os error 49)"
        );
    }
}
