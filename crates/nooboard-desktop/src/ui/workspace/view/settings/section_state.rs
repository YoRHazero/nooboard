#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::ui::workspace::view) enum SettingsStatus {
    Current,
    Modified,
    Applying,
    Review,
    Error,
    Stale,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::ui::workspace::view) enum StorageSettingField {
    HistoryWindowDays,
    DedupWindowDays,
    MaxTextBytes,
    GcBatchSize,
}

impl StorageSettingField {
    pub(super) fn step(self) -> usize {
        match self {
            Self::HistoryWindowDays => 1,
            Self::DedupWindowDays => 1,
            Self::MaxTextBytes => 1024,
            Self::GcBatchSize => 64,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::ui::workspace::view) enum SettingsSectionPhase {
    Normal,
    Applying,
    Error(String),
    Stale,
}

#[derive(Clone, PartialEq, Eq)]
pub(in crate::ui::workspace::view) struct SettingsSection<T> {
    pub(super) baseline: T,
    pub(super) draft: T,
    pub(super) phase: SettingsSectionPhase,
}

impl<T: Clone + PartialEq + Eq> SettingsSection<T> {
    pub(super) fn new(current: T) -> Self {
        Self {
            baseline: current.clone(),
            draft: current,
            phase: SettingsSectionPhase::Normal,
        }
    }

    pub(super) fn is_dirty(&self) -> bool {
        self.draft != self.baseline
    }

    pub(super) fn reset(&mut self) {
        self.draft = self.baseline.clone();
        self.phase = SettingsSectionPhase::Normal;
    }

    pub(super) fn mark_edited(&mut self) {
        if !matches!(self.phase, SettingsSectionPhase::Applying) {
            self.phase = SettingsSectionPhase::Normal;
        }
    }

    pub(super) fn begin_apply(&mut self) {
        self.phase = SettingsSectionPhase::Applying;
    }

    pub(super) fn mark_error(&mut self, message: String) {
        self.phase = SettingsSectionPhase::Error(message);
    }

    pub(super) fn sync_from_live(&mut self, current: T) {
        if self.draft == current {
            self.baseline = current.clone();
            self.draft = current;
            self.phase = SettingsSectionPhase::Normal;
            return;
        }

        if matches!(self.phase, SettingsSectionPhase::Applying) {
            self.baseline = current;
            return;
        }

        if !self.is_dirty() {
            self.baseline = current.clone();
            self.draft = current;
            self.phase = SettingsSectionPhase::Normal;
            return;
        }

        if self.baseline != current {
            self.baseline = current;
            self.phase = SettingsSectionPhase::Stale;
        }
    }
}

pub(in crate::ui::workspace::view) fn settings_section_status<T: Clone + PartialEq + Eq>(
    section: &SettingsSection<T>,
    has_validation_issues: bool,
) -> SettingsStatus {
    if matches!(section.phase, SettingsSectionPhase::Applying) {
        SettingsStatus::Applying
    } else if has_validation_issues {
        SettingsStatus::Review
    } else if matches!(section.phase, SettingsSectionPhase::Error(_)) {
        SettingsStatus::Error
    } else if matches!(section.phase, SettingsSectionPhase::Stale) {
        SettingsStatus::Stale
    } else if section.is_dirty() {
        SettingsStatus::Modified
    } else {
        SettingsStatus::Current
    }
}

#[cfg(test)]
mod tests {
    use super::super::snapshot::{
        ClipboardSettingsValue, NetworkSettingsValue, StorageSettingsValue, TransferSettingsValue,
    };
    use super::*;

    #[test]
    fn sync_from_live_marks_dirty_section_stale_when_baseline_changes() {
        let mut section = SettingsSection::new(NetworkSettingsValue {
            network_enabled: true,
            mdns_enabled: true,
            manual_peers: vec![],
        });
        section.draft.network_enabled = false;

        section.sync_from_live(NetworkSettingsValue {
            network_enabled: true,
            mdns_enabled: false,
            manual_peers: vec![],
        });

        assert_eq!(section.baseline.mdns_enabled, false);
        assert!(section.is_dirty());
        assert_eq!(section.phase, SettingsSectionPhase::Stale);
    }

    #[test]
    fn sync_from_live_clears_applying_section_when_live_matches_draft() {
        let mut section = SettingsSection::new(ClipboardSettingsValue {
            local_capture_enabled: true,
        });
        section.draft.local_capture_enabled = false;
        section.begin_apply();

        section.sync_from_live(ClipboardSettingsValue {
            local_capture_enabled: false,
        });

        assert!(!section.is_dirty());
        assert_eq!(section.phase, SettingsSectionPhase::Normal);
    }

    #[test]
    fn sync_from_live_keeps_apply_phase_during_partial_network_updates() {
        let mut section = SettingsSection::new(NetworkSettingsValue {
            network_enabled: true,
            mdns_enabled: true,
            manual_peers: vec![],
        });
        section.draft = NetworkSettingsValue {
            network_enabled: false,
            mdns_enabled: false,
            manual_peers: vec!["127.0.0.1:24001".parse().unwrap()],
        };
        section.begin_apply();

        section.sync_from_live(NetworkSettingsValue {
            network_enabled: true,
            mdns_enabled: false,
            manual_peers: vec![],
        });

        assert_eq!(section.phase, SettingsSectionPhase::Applying);
        assert_eq!(section.baseline.mdns_enabled, false);
        assert!(section.is_dirty());
    }

    #[test]
    fn settings_section_status_prefers_validation_over_dirty() {
        let section = SettingsSection::new(StorageSettingsValue {
            db_root: "/tmp/db".into(),
            history_window_days: 7,
            dedup_window_days: 14,
            max_text_bytes: 4096,
            gc_batch_size: 64,
        });

        assert_eq!(
            settings_section_status(&section, true),
            SettingsStatus::Review
        );
    }

    #[test]
    fn settings_section_status_reports_error_phase() {
        let mut section = SettingsSection::new(TransferSettingsValue {
            download_dir: "/tmp/downloads".into(),
        });
        section.mark_error("boom".to_string());

        assert_eq!(
            settings_section_status(&section, false),
            SettingsStatus::Error
        );
    }
}
