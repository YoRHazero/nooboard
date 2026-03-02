mod feed;
mod system_core;

use gpui::{Div, ParentElement, Styled, div, px};

use super::{WorkspaceView, components::summary_card};
use crate::ui::theme;
use gpui_component::{IconName, StyledExt};

impl WorkspaceView {
    pub(super) fn home_page(&self) -> Div {
        let app = &self.state.app;

        div()
            .v_flex()
            .w_full()
            .flex_shrink_0()
            .gap(px(18.0))
            .child(
                div()
                    .h_flex()
                    .items_start()
                    .gap(px(16.0))
                    .child(summary_card(
                        "home-peers-card",
                        "Online Peers",
                        app.online_peers.to_string(),
                        format!("{} manual peers pinned for bootstrap", app.manual_peers),
                        IconName::Globe,
                        theme::accent_cyan(),
                    ))
                    .child(summary_card(
                        "home-inbox-card",
                        "Pending Files",
                        app.pending_files.len().to_string(),
                        "file decisions waiting in the intake queue",
                        IconName::Inbox,
                        theme::accent_amber(),
                    ))
                    .child(summary_card(
                        "home-history-card",
                        "Today History",
                        app.today_history_count.to_string(),
                        "clipboard events indexed in local history today",
                        IconName::Copy,
                        theme::accent_blue(),
                    )),
            )
            .child(self.system_core_card())
            .child(self.recent_activity_card())
            .child(self.transfer_queue_card())
    }
}
