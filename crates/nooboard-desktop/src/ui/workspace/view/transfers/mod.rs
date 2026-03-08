mod components;
mod downloads;
mod header;
mod page_state;
mod uploads;

use gpui::{Context, Div, ParentElement, Styled, div, px};
use gpui_component::StyledExt;

use super::WorkspaceView;

pub(super) use page_state::TransfersPageState;

impl WorkspaceView {
    pub(super) fn transfers_page(&self, cx: &mut Context<Self>) -> Div {
        div()
            .w_full()
            .v_flex()
            .gap(px(18.0))
            .child(self.transfers_header())
            .child(self.transfers_target_panel(cx))
            .child(self.transfers_upload_panel(cx))
            .child(self.transfers_download_panel(cx))
    }
}
