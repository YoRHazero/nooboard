use super::*;

impl WorkspaceView {
    pub(super) fn clipboard_header(&self) -> Div {
        let clipboard = &self.state.app.clipboard;

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
                            .child("live + stored"),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .gap(px(8.0))
                    .child(clipboard_metric_chip(
                        "Targets",
                        format!(
                            "{}/{}",
                            self.clipboard_page.selected_target_count(),
                            clipboard.targets.len()
                        ),
                        theme::accent_cyan(),
                    ))
                    .child(clipboard_metric_chip(
                        "History",
                        clipboard.history_items().count().to_string(),
                        theme::accent_amber(),
                    )),
            )
    }
}
