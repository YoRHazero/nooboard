mod components;
mod detail;
mod header;
mod history;
mod page_state;
mod targets;

use gpui::{
    AnyElement, Context, Div, Hsla, InteractiveElement, IntoElement, ParentElement, Styled,
    Window, div, px,
};
use gpui_component::scroll::ScrollableElement;
use gpui_component::StyledExt;
use uuid::Uuid;

use crate::{
    state::{
        ClipboardHistoryPage, ClipboardStore, ClipboardTarget, ClipboardTargetStatus,
        ClipboardTextItem, ClipboardTextOrigin, ClipboardTextResidency,
    },
    ui::theme,
};

pub(super) use page_state::ClipboardPageState;

use self::components::{
    clipboard_action_button, clipboard_action_with_tooltip, clipboard_badge,
    clipboard_history_item_body, clipboard_history_item_shell, clipboard_metric_chip,
    clipboard_panel_header, clipboard_panel_shell, clipboard_target_chip,
};

use super::WorkspaceView;

const CLIPBOARD_HISTORY_WIDTH: f32 = 306.0;
const CLIPBOARD_TEXT_PANEL_MIN_HEIGHT: f32 = 376.0;

impl WorkspaceView {
    pub(super) fn clipboard_page(&self, cx: &mut Context<Self>) -> Div {
        let active_item = self.clipboard_page.active_item(&self.state.app.clipboard);

        div()
            .w_full()
            .v_flex()
            .gap(px(18.0))
            .child(self.clipboard_header())
            .child(self.clipboard_targets_panel(cx))
            .child(
                div()
                    .flex()
                    .min_h(px(640.0))
                    .gap(px(18.0))
                    .items_stretch()
                    .child(self.clipboard_history_panel(cx))
                    .child(self.clipboard_detail_panel(&active_item, cx)),
            )
    }

    fn clipboard_item_accent(&self, item: &ClipboardTextItem) -> Hsla {
        match item.origin {
            ClipboardTextOrigin::Local => theme::accent_green(),
            ClipboardTextOrigin::Remote => theme::accent_blue(),
        }
    }

    fn clipboard_origin_label(&self, item: &ClipboardTextItem) -> String {
        match item.origin {
            ClipboardTextOrigin::Local => "Local".to_string(),
            ClipboardTextOrigin::Remote => "Remote".to_string(),
        }
    }

    fn clipboard_residency_label(&self, item: &ClipboardTextItem) -> &'static str {
        match item.residency {
            ClipboardTextResidency::Live => "Live",
            ClipboardTextResidency::History => "History",
        }
    }

    fn clipboard_short_event_id(&self, event_id: Uuid) -> String {
        event_id.simple().to_string().chars().take(8).collect()
    }
}
