use crate::workspace::view_state::SettingsClipboardViewState;

pub(super) struct ClipboardSettingsState {
    local_capture_enabled: bool,
    synced_local_capture_enabled: Option<bool>,
}

impl ClipboardSettingsState {
    pub(super) fn new() -> Self {
        Self {
            local_capture_enabled: true,
            synced_local_capture_enabled: None,
        }
    }

    pub(super) fn sync_from_workspace(&mut self, page: &SettingsClipboardViewState) {
        if self
            .synced_local_capture_enabled
            .is_none_or(|previous| self.local_capture_enabled == previous)
        {
            self.local_capture_enabled = page.local_capture_enabled;
        }
        self.synced_local_capture_enabled = Some(page.local_capture_enabled);
    }

    pub(super) fn local_capture_enabled(&self) -> bool {
        self.local_capture_enabled
    }

    pub(super) fn toggle_local_capture_enabled(&mut self) {
        self.local_capture_enabled = !self.local_capture_enabled;
    }

    pub(super) fn reset_from_workspace(&mut self, page: &SettingsClipboardViewState) {
        self.local_capture_enabled = page.local_capture_enabled;
        self.synced_local_capture_enabled = Some(page.local_capture_enabled);
    }
}
