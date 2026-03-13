mod components;
mod recent_activity;
mod snapshot;
mod system_core;

use gpui::{Context, Div, ParentElement, Styled, div, px};

use super::{WorkspaceView, shared::HOME_CONTENT_WIDTH};
use gpui_component::StyledExt;
use snapshot::build_home_snapshot;

impl WorkspaceView {
    pub(super) fn home_page(&self, cx: &mut Context<Self>) -> Div {
        let live_store = self.live_store.read(cx);
        let snapshot = build_home_snapshot(&live_store);

        div().w_full().flex().justify_center().child(
            div()
                .w(px(HOME_CONTENT_WIDTH))
                .max_w_full()
                .v_flex()
                .flex_shrink_0()
                .gap(px(18.0))
                .child(self.system_core_card(&snapshot.system_core, cx))
                .child(self.recent_activity_card(&snapshot.recent_activity, cx)),
        )
    }
}
