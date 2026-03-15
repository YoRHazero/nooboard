mod actions;
mod components;
mod detail;
mod header;
mod history;
mod state;
mod view_state;

use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px};
use nooboard_core::ClipboardRecordSource;

use crate::ui::theme;

use super::{WorkspaceRenderModel, WorkspaceView};

pub(in crate::ui::workspace) use state::ClipboardPageState;
use view_state::build_clipboard_page_view_state;

const CLIPBOARD_HISTORY_WIDTH: f32 = 312.0;
const CLIPBOARD_DETAIL_MIN_WIDTH: f32 = 456.0;
const CLIPBOARD_PANEL_MIN_HEIGHT: f32 = 420.0;

impl WorkspaceView {
    pub(super) fn clipboard_source_accent(&self, source: ClipboardRecordSource) -> gpui::Hsla {
        match source {
            ClipboardRecordSource::LocalCapture => theme::accent_green(),
            ClipboardRecordSource::RemoteSync => theme::accent_blue(),
            ClipboardRecordSource::UserSubmit => theme::accent_cyan(),
        }
    }

    pub(super) fn clipboard_page(
        &self,
        model: &WorkspaceRenderModel,
        cx: &Context<Self>,
    ) -> Vec<AnyElement> {
        let snapshot = build_clipboard_page_view_state(
            model.page.as_ref().map(|page| &page.clipboard),
            &self.clipboard,
            cx,
        );

        vec![
            self.clipboard_header(&snapshot).into_any_element(),
            self.clipboard_targets_panel(&snapshot, cx)
                .into_any_element(),
            div()
                .w_full()
                .flex()
                .flex_wrap()
                .gap(px(18.0))
                .items_stretch()
                .child(self.clipboard_history_panel(&snapshot, cx))
                .child(self.clipboard_detail_panel(&snapshot, cx))
                .into_any_element(),
            if snapshot.page_ready {
                div()
                    .text_size(px(11.0))
                    .text_color(theme::fg_muted())
                    .child("Clipboard history is rendered from core queries; latest status still follows WorkspaceSnapshot.")
                    .into_any_element()
            } else {
                div()
                    .text_size(px(11.0))
                    .text_color(theme::fg_muted())
                    .child("Clipboard workspace is still loading.")
                    .into_any_element()
            },
        ]
    }
}
