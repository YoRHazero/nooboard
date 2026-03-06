use uuid::Uuid;

#[derive(Clone)]
pub struct ClipboardStore {
    pub targets: Vec<ClipboardTarget>,
    pub default_selected_target_noob_ids: Vec<String>,
    pub local_live: ClipboardTextItem,
    pub latest_remote_live: Option<ClipboardTextItem>,
    pub history_pages: Vec<ClipboardHistoryPage>,
}

impl ClipboardStore {
    pub fn latest_live_item(&self) -> &ClipboardTextItem {
        match self.latest_remote_live.as_ref() {
            Some(remote) if remote.recorded_at_ms > self.local_live.recorded_at_ms => remote,
            _ => &self.local_live,
        }
    }

    pub fn history_items(&self) -> impl Iterator<Item = &ClipboardTextItem> {
        self.history_pages.iter().flat_map(|page| page.items.iter())
    }
}

#[derive(Clone)]
pub struct ClipboardHistoryPage {
    pub items: Vec<ClipboardTextItem>,
}

impl ClipboardHistoryPage {
    pub fn new(items: Vec<ClipboardTextItem>) -> Self {
        Self { items }
    }
}

#[derive(Clone)]
pub struct ClipboardTextItem {
    pub event_id: Uuid,
    pub device_id: String,
    pub content: String,
    pub recorded_at_ms: i64,
    pub recorded_at_label: String,
    pub origin: ClipboardTextOrigin,
    pub residency: ClipboardTextResidency,
}

impl ClipboardTextItem {
    pub fn local_live(
        event_id: Uuid,
        device_id: impl Into<String>,
        recorded_at_ms: i64,
        recorded_at_label: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self::new(
            event_id,
            device_id,
            recorded_at_ms,
            recorded_at_label,
            content,
            ClipboardTextOrigin::Local,
            ClipboardTextResidency::Live,
        )
    }

    pub fn remote_live(
        event_id: Uuid,
        device_id: impl Into<String>,
        recorded_at_ms: i64,
        recorded_at_label: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self::new(
            event_id,
            device_id,
            recorded_at_ms,
            recorded_at_label,
            content,
            ClipboardTextOrigin::Remote,
            ClipboardTextResidency::Live,
        )
    }

    pub fn local_history(
        event_id: Uuid,
        device_id: impl Into<String>,
        recorded_at_ms: i64,
        recorded_at_label: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self::new(
            event_id,
            device_id,
            recorded_at_ms,
            recorded_at_label,
            content,
            ClipboardTextOrigin::Local,
            ClipboardTextResidency::History,
        )
    }

    pub fn remote_history(
        event_id: Uuid,
        device_id: impl Into<String>,
        recorded_at_ms: i64,
        recorded_at_label: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self::new(
            event_id,
            device_id,
            recorded_at_ms,
            recorded_at_label,
            content,
            ClipboardTextOrigin::Remote,
            ClipboardTextResidency::History,
        )
    }

    pub fn preview_text(&self, limit: usize) -> String {
        let mut preview = String::new();
        let mut length = 0usize;

        for ch in self.content.chars() {
            if length >= limit {
                preview.push_str("...");
                return preview;
            }

            preview.push(ch);
            length += 1;
        }

        preview
    }

    pub fn can_write_to_clipboard(&self) -> bool {
        self.origin == ClipboardTextOrigin::Remote
            || self.residency == ClipboardTextResidency::History
    }

    pub fn can_broadcast(&self) -> bool {
        self.origin == ClipboardTextOrigin::Local
            || self.residency == ClipboardTextResidency::History
    }

    pub fn can_store(&self) -> bool {
        self.origin == ClipboardTextOrigin::Remote && self.residency == ClipboardTextResidency::Live
    }

    pub fn can_delete(&self) -> bool {
        self.residency == ClipboardTextResidency::History
    }

    fn new(
        event_id: Uuid,
        device_id: impl Into<String>,
        recorded_at_ms: i64,
        recorded_at_label: impl Into<String>,
        content: impl Into<String>,
        origin: ClipboardTextOrigin,
        residency: ClipboardTextResidency,
    ) -> Self {
        Self {
            event_id,
            device_id: device_id.into(),
            content: content.into(),
            recorded_at_ms,
            recorded_at_label: recorded_at_label.into(),
            origin,
            residency,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ClipboardTextOrigin {
    Local,
    Remote,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ClipboardTextResidency {
    Live,
    History,
}

#[derive(Clone)]
pub struct ClipboardTarget {
    pub noob_id: String,
    pub device_id: String,
    pub status: ClipboardTargetStatus,
}

impl ClipboardTarget {
    pub fn connected(noob_id: impl Into<String>, device_id: impl Into<String>) -> Self {
        Self {
            noob_id: noob_id.into(),
            device_id: device_id.into(),
            status: ClipboardTargetStatus::Connected,
        }
    }

    pub fn offline(noob_id: impl Into<String>, device_id: impl Into<String>) -> Self {
        Self {
            noob_id: noob_id.into(),
            device_id: device_id.into(),
            status: ClipboardTargetStatus::Offline,
        }
    }

    pub fn is_connected(&self) -> bool {
        self.status == ClipboardTargetStatus::Connected
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ClipboardTargetStatus {
    Connected,
    Offline,
}
