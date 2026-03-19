mod actions;
mod components;
mod detail;
mod header;
mod history;
mod state;
mod view_state;

use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div, px};
use gpui_component::StyledExt;
use nooboard_core::ClipboardRecordSource;

use crate::ui::theme;

use super::{
    WorkspaceRenderModel, WorkspaceView,
    shared::{SIDEBAR_WIDTH, TRANSFER_RAIL_COLLAPSED_WIDTH, TRANSFER_RAIL_WIDTH},
};

pub(in crate::ui::workspace) use state::ClipboardPageState;
use view_state::build_clipboard_page_view_state;

const CLIPBOARD_HISTORY_WIDTH: f32 = 312.0;
const CLIPBOARD_DETAIL_MIN_WIDTH: f32 = 360.0;
const CLIPBOARD_PANEL_MIN_HEIGHT: f32 = 420.0;
const CLIPBOARD_PAGE_MIN_WIDTH: f32 = CLIPBOARD_HISTORY_WIDTH + 18.0 + CLIPBOARD_DETAIL_MIN_WIDTH;
const WORKSPACE_SHELL_HORIZONTAL_PADDING: f32 = 36.0;
const WORKSPACE_SHELL_HORIZONTAL_GAPS: f32 = 36.0;
const MAIN_VIEWPORT_HORIZONTAL_PADDING: f32 = 48.0;

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
        window: &Window,
        cx: &Context<Self>,
    ) -> Vec<AnyElement> {
        let snapshot = build_clipboard_page_view_state(
            model.page.as_ref().map(|page| &page.clipboard),
            &self.clipboard,
            cx,
        );
        let page_width = self.clipboard_page_width(window);

        vec![
            div()
                .w(page_width)
                .min_w(px(CLIPBOARD_PAGE_MIN_WIDTH))
                .flex_shrink_0()
                .v_flex()
                .gap(px(18.0))
                .child(self.clipboard_header(&snapshot))
                .child(self.clipboard_targets_panel(&snapshot, cx))
                .child(
                    div()
                        .w_full()
                        .flex()
                        .gap(px(18.0))
                        .items_stretch()
                        .child(self.clipboard_history_panel(&snapshot, cx))
                        .child(self.clipboard_detail_panel(&snapshot, cx)),
                )
                .child(if snapshot.page_ready {
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
                })
                .into_any_element(),
        ]
    }

    fn clipboard_page_width(&self, window: &Window) -> gpui::Pixels {
        let rail_width = if self.transfer_rail_expanded {
            TRANSFER_RAIL_WIDTH
        } else {
            TRANSFER_RAIL_COLLAPSED_WIDTH
        };
        let available_width = window.viewport_size().width
            - px(WORKSPACE_SHELL_HORIZONTAL_PADDING
                + WORKSPACE_SHELL_HORIZONTAL_GAPS
                + SIDEBAR_WIDTH
                + rail_width
                + MAIN_VIEWPORT_HORIZONTAL_PADDING);

        if available_width < px(CLIPBOARD_PAGE_MIN_WIDTH) {
            px(CLIPBOARD_PAGE_MIN_WIDTH)
        } else {
            available_width
        }
    }
}
