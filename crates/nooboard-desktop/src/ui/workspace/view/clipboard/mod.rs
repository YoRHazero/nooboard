mod actions;
mod components;
mod detail;
mod guards;
mod header;
mod history;
mod page_state;
mod snapshot;
mod targets;

use gpui::{Context, Div, Hsla, IntoElement, ParentElement, Styled, div, px};
use gpui_component::StyledExt;
use gpui_component::scroll::ScrollableElement;
use nooboard_app::{ClipboardRecord, ClipboardRecordSource};

use crate::ui::theme;

pub(super) use page_state::ClipboardPageState;

use self::components::{
    clipboard_badge, clipboard_history_item_body, clipboard_history_item_shell,
    clipboard_metric_chip, clipboard_panel_header, clipboard_panel_shell, clipboard_target_chip,
};
use self::snapshot::{ClipboardSnapshot, build_clipboard_snapshot, clipboard_source_label};

use super::WorkspaceView;

const CLIPBOARD_HISTORY_WIDTH: f32 = 306.0;
const CLIPBOARD_TEXT_PANEL_MIN_HEIGHT: f32 = 376.0;

impl WorkspaceView {
    pub(super) fn clipboard_page(&self, cx: &mut Context<Self>) -> Div {
        let snapshot = self.clipboard_snapshot(cx);

        div()
            .w_full()
            .v_flex()
            .gap(px(18.0))
            .child(self.clipboard_header(&snapshot))
            .child(self.clipboard_targets_panel(&snapshot, cx))
            .child(
                div()
                    .flex()
                    .min_h(px(640.0))
                    .gap(px(18.0))
                    .items_stretch()
                    .child(self.clipboard_history_panel(&snapshot, cx))
                    .child(self.clipboard_detail_panel(&snapshot, cx)),
            )
    }

    fn clipboard_snapshot(&self, cx: &mut Context<Self>) -> ClipboardSnapshot {
        let store = self.live_store.read(cx);
        build_clipboard_snapshot(&store, &self.clipboard_page, cx)
    }

    pub(super) fn selected_clipboard_record(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<ClipboardRecord> {
        self.clipboard_snapshot(cx).selected_record
    }

    pub(super) fn clipboard_source_accent(&self, source: ClipboardRecordSource) -> Hsla {
        match source {
            ClipboardRecordSource::LocalCapture => theme::accent_green(),
            ClipboardRecordSource::RemoteSync => theme::accent_blue(),
            ClipboardRecordSource::UserSubmit => theme::accent_cyan(),
        }
    }

    pub(super) fn clipboard_short_event_id(&self, event_id: nooboard_app::EventId) -> String {
        event_id
            .as_uuid()
            .simple()
            .to_string()
            .chars()
            .take(8)
            .collect()
    }
}
