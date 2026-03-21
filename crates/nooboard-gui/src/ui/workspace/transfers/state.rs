use std::{collections::BTreeSet, fs, path::PathBuf, time::SystemTime};

use nooboard_core::{SessionId, TransferTicket};

use crate::workspace::view_state::TransfersPageViewState;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StagedFileSource {
    Dropped,
    Browsed,
}

impl StagedFileSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::Dropped => "Dropped",
            Self::Browsed => "Browsed",
        }
    }
}

#[derive(Clone)]
pub struct StagedTransferFile {
    pub id: String,
    pub file_name: String,
    pub file_path: PathBuf,
    pub size_label: String,
    pub modified_at_label: String,
    pub source: StagedFileSource,
}

pub(in crate::ui::workspace) struct TransfersPageState {
    selected_session_ids: BTreeSet<SessionId>,
    staged_files: Vec<StagedTransferFile>,
    pending_tickets: BTreeSet<String>,
    send_in_flight: bool,
    feedback: Option<String>,
    next_staged_file_id: usize,
}

impl TransfersPageState {
    pub(in crate::ui::workspace) fn new() -> Self {
        Self {
            selected_session_ids: BTreeSet::new(),
            staged_files: Vec::new(),
            pending_tickets: BTreeSet::new(),
            send_in_flight: false,
            feedback: None,
            next_staged_file_id: 1,
        }
    }

    pub(in crate::ui::workspace) fn sync_from_workspace(
        &mut self,
        page: Option<&TransfersPageViewState>,
    ) {
        let Some(page) = page else {
            return;
        };
        self.selected_session_ids
            .retain(|id| page.available_targets.iter().any(|target| target.id == *id));
        self.pending_tickets.retain(|ticket| {
            let key = |ticket: TransferTicket| ticket_key(ticket);
            page.incoming.iter().any(|item| key(item.ticket) == *ticket)
                || page.active.iter().any(|item| key(item.ticket) == *ticket)
        });
    }

    pub(in crate::ui::workspace) fn selected_session_ids(&self) -> &BTreeSet<SessionId> {
        &self.selected_session_ids
    }

    pub(in crate::ui::workspace) fn staged_files(&self) -> &[StagedTransferFile] {
        &self.staged_files
    }

    pub(in crate::ui::workspace) fn send_in_flight(&self) -> bool {
        self.send_in_flight
    }

    pub(in crate::ui::workspace) fn feedback(&self) -> Option<&String> {
        self.feedback.as_ref()
    }

    pub(in crate::ui::workspace) fn pending_ticket(&self, ticket: TransferTicket) -> bool {
        self.pending_tickets.contains(&ticket_key(ticket))
    }

    pub(in crate::ui::workspace) fn toggle_target(&mut self, id: SessionId) {
        if self.selected_session_ids.contains(&id) {
            self.selected_session_ids.remove(&id);
        } else {
            self.selected_session_ids.insert(id);
        }
        let count = self.selected_session_ids.len();
        self.feedback = Some(format!(
            "{} device{} selected.",
            count,
            if count == 1 { "" } else { "s" }
        ));
    }

    pub(in crate::ui::workspace) fn queue_paths(
        &mut self,
        paths: Vec<PathBuf>,
        source: StagedFileSource,
    ) {
        let mut existing = self
            .staged_files
            .iter()
            .map(|item| item.file_path.clone())
            .collect::<BTreeSet<_>>();
        let mut staged_now = 0usize;

        for path in paths {
            if existing.contains(&path) {
                continue;
            }

            let Some(file_name) = path
                .file_name()
                .map(|value| value.to_string_lossy().into_owned())
            else {
                continue;
            };
            let Ok(metadata) = fs::metadata(&path) else {
                continue;
            };
            if !metadata.is_file() {
                continue;
            }

            self.staged_files.push(StagedTransferFile {
                id: format!("transfer-stage-{}", self.next_staged_file_id),
                file_name,
                file_path: path.clone(),
                size_label: bytes_to_label(metadata.len()),
                modified_at_label: metadata
                    .modified()
                    .ok()
                    .map(system_time_to_clock_label)
                    .unwrap_or_else(|| "unknown".to_string()),
                source,
            });
            self.next_staged_file_id += 1;
            existing.insert(path);
            staged_now += 1;
        }

        if staged_now > 0 {
            self.feedback = Some(format!(
                "Added {} file{}.",
                staged_now,
                if staged_now == 1 { "" } else { "s" }
            ));
        }
    }

    pub(in crate::ui::workspace) fn remove_staged_file(&mut self, staged_file_id: &str) {
        let before = self.staged_files.len();
        self.staged_files.retain(|item| item.id != staged_file_id);
        if self.staged_files.len() != before {
            self.feedback = Some("Removed file.".to_string());
        }
    }

    pub(in crate::ui::workspace) fn begin_send(&mut self) {
        self.send_in_flight = true;
        self.feedback = Some("Sending files...".to_string());
    }

    pub(in crate::ui::workspace) fn finish_send(&mut self, message: String) {
        self.send_in_flight = false;
        self.staged_files.clear();
        self.feedback = Some(message);
    }

    pub(in crate::ui::workspace) fn fail_send(&mut self, message: String) {
        self.send_in_flight = false;
        self.feedback = Some(message);
    }

    pub(in crate::ui::workspace) fn mark_ticket_pending(
        &mut self,
        ticket: TransferTicket,
        message: String,
    ) {
        self.pending_tickets.insert(ticket_key(ticket));
        self.feedback = Some(message);
    }

    pub(in crate::ui::workspace) fn clear_ticket_pending(
        &mut self,
        ticket: TransferTicket,
        message: String,
    ) {
        self.pending_tickets.remove(&ticket_key(ticket));
        self.feedback = Some(message);
    }
}

fn ticket_key(ticket: TransferTicket) -> String {
    format!("{}:{}", ticket.session_id, ticket.raw_id)
}

fn bytes_to_label(value: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let value = value as f64;
    if value >= GB {
        format!("{:.1} GB", value / GB)
    } else if value >= MB {
        format!("{:.1} MB", value / MB)
    } else if value >= KB {
        format!("{:.1} KB", value / KB)
    } else {
        format!("{} B", value as u64)
    }
}

fn system_time_to_clock_label(timestamp: SystemTime) -> String {
    let Ok(duration) = timestamp.duration_since(SystemTime::UNIX_EPOCH) else {
        return "unknown".to_string();
    };
    let seconds = duration.as_secs();
    let hours = (seconds / 3600) % 24;
    let minutes = (seconds / 60) % 60;
    let secs = seconds % 60;
    format!("{hours:02}:{minutes:02}:{secs:02}")
}
