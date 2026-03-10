use super::*;
use gpui_component::StyledExt;

impl WorkspaceView {
    pub(super) fn clipboard_header(&self, snapshot: &ClipboardSnapshot) -> Div {
        clipboard_panel_shell()
            .rounded(px(24.0))
            .h_flex()
            .items_center()
            .justify_between()
            .gap(px(14.0))
            .p(px(18.0))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(22.0))
                            .font_semibold()
                            .text_color(theme::fg_primary())
                            .child("Clipboard"),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme::fg_muted())
                            .child("committed history"),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .gap(px(8.0))
                    .child(clipboard_metric_chip(
                        "Peers",
                        snapshot.connected_target_count.to_string(),
                        theme::accent_cyan(),
                    ))
                    .child(clipboard_metric_chip(
                        "History",
                        snapshot.loaded_history_count.to_string(),
                        theme::accent_amber(),
                    ))
                    .child(clipboard_metric_chip(
                        "Broadcast",
                        match snapshot.broadcast_scope {
                            page_state::ClipboardBroadcastScope::AllConnected => {
                                "All connected".to_string()
                            }
                            page_state::ClipboardBroadcastScope::SelectedPeers => {
                                format!("{} selected", snapshot.selected_target_count)
                            }
                        },
                        theme::accent_blue(),
                    )),
            )
    }
}
