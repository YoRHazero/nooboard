use nooboard_core::{ClipboardRecord, ClipboardRecordSource, WorkspaceSnapshot};

use super::WorkspaceMetricViewState;

#[derive(Clone)]
pub struct HomePageViewState {
    pub metrics: Vec<WorkspaceMetricViewState>,
    pub latest_clipboard_preview: String,
    pub latest_clipboard_event_id: Option<String>,
    pub latest_clipboard_source: Option<String>,
}

pub(super) fn build_home_page_view_state(
    snapshot: &WorkspaceSnapshot,
    latest_record: Option<&ClipboardRecord>,
) -> HomePageViewState {
    HomePageViewState {
        metrics: vec![
            WorkspaceMetricViewState {
                label: "Network",
                value: super::network::network_status_label(&snapshot.network.status),
            },
            WorkspaceMetricViewState {
                label: "LAN Peers",
                value: snapshot.network.lan_peers.len().to_string(),
            },
            WorkspaceMetricViewState {
                label: "Direct Seeds",
                value: snapshot.network.direct_seeds.len().to_string(),
            },
            WorkspaceMetricViewState {
                label: "Requests",
                value: snapshot.network.pending_direct_requests.len().to_string(),
            },
            WorkspaceMetricViewState {
                label: "Sessions",
                value: snapshot.network.sessions.len().to_string(),
            },
            WorkspaceMetricViewState {
                label: "Transfers",
                value: format!(
                    "{} active / {} pending / {} complete",
                    snapshot.network.transfers.active.len(),
                    snapshot.network.transfers.incoming_pending.len(),
                    snapshot.network.transfers.recent_completed.len()
                ),
            },
            WorkspaceMetricViewState {
                label: "Endpoint",
                value: snapshot
                    .local_connection
                    .device_endpoint
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "Unavailable".to_string()),
            },
        ],
        latest_clipboard_preview: latest_record
            .map(|record| clipboard_preview(&record.content))
            .unwrap_or_else(|| "No committed clipboard record yet.".to_string()),
        latest_clipboard_event_id: latest_record.map(|record| record.event_id.to_string()),
        latest_clipboard_source: latest_record
            .map(|record| clipboard_source_label(record.source).to_string()),
    }
}

fn clipboard_preview(content: &str) -> String {
    let preview = content.replace('\n', " ");
    let mut chars = preview.chars();
    let compact = chars.by_ref().take(140).collect::<String>();
    if chars.next().is_some() {
        format!("{compact}…")
    } else {
        compact
    }
}

fn clipboard_source_label(source: ClipboardRecordSource) -> &'static str {
    match source {
        ClipboardRecordSource::LocalCapture => "Local Capture",
        ClipboardRecordSource::RemoteSync => "Remote Sync",
        ClipboardRecordSource::UserSubmit => "User Submit",
    }
}
