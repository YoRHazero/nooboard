mod detail;
mod header;
mod history;
mod state;
mod targets;

use gpui::{
    AnyElement, AnyView, App, Context, Div, Hsla, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_component::scroll::ScrollableElement;
use gpui_component::tooltip::Tooltip;
use gpui_component::{Disableable, StyledExt};
use uuid::Uuid;

use crate::{
    state::{
        ClipboardHistoryPage, ClipboardStore, ClipboardTarget, ClipboardTargetStatus,
        ClipboardTextItem, ClipboardTextOrigin, ClipboardTextResidency,
    },
    ui::theme,
};

pub(super) use state::ClipboardPageState;

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

    fn clipboard_themed_tooltip(text: String, window: &mut Window, cx: &mut App) -> AnyView {
        Tooltip::new(text)
            .bg(theme::bg_panel())
            .text_color(theme::fg_primary())
            .border_color(theme::border_base())
            .build(window, cx)
    }

    fn clipboard_badge(&self, label: String, accent: Hsla) -> Div {
        div()
            .px(px(10.0))
            .py(px(6.0))
            .rounded(px(999.0))
            .bg(accent.opacity(0.14))
            .border_1()
            .border_color(accent.opacity(0.28))
            .text_size(px(10.0))
            .font_semibold()
            .text_color(accent)
            .child(label)
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
