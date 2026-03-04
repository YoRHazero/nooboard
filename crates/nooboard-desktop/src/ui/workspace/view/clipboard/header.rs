use super::*;

impl WorkspaceView {
    pub(super) fn clipboard_header(&self) -> Div {
        let clipboard = &self.state.app.clipboard;

        div()
            .h_flex()
            .items_center()
            .justify_between()
            .gap(px(14.0))
            .p(px(18.0))
            .bg(theme::bg_panel())
            .border_1()
            .border_color(theme::border_base())
            .rounded(px(24.0))
            .shadow_xs()
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
                    .child(self.clipboard_metric_chip(
                        "Targets",
                        format!(
                            "{}/{}",
                            self.clipboard_page.selected_target_count(),
                            clipboard.targets.len()
                        ),
                        theme::accent_cyan(),
                    ))
                    .child(self.clipboard_metric_chip(
                        "History",
                        clipboard.history_items().count().to_string(),
                        theme::accent_amber(),
                    )),
            )
    }

    fn clipboard_metric_chip(&self, label: &str, value: String, accent: Hsla) -> Div {
        div()
            .h_flex()
            .items_center()
            .gap(px(8.0))
            .px(px(12.0))
            .py(px(9.0))
            .bg(theme::bg_console())
            .border_1()
            .border_color(accent.opacity(0.22))
            .rounded(px(16.0))
            .child(
                div()
                    .text_size(px(10.0))
                    .font_semibold()
                    .text_color(accent)
                    .child(label.to_string()),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .font_semibold()
                    .text_color(theme::fg_primary())
                    .child(value),
            )
    }
}
