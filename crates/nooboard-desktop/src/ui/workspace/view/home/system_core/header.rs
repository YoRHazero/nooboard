use super::*;

impl WorkspaceView {
    pub(super) fn system_core_header(&self) -> Div {
        let accent = theme::accent_cyan();

        div()
            .h_flex()
            .items_center()
            .justify_between()
            .gap(px(16.0))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap(px(14.0))
                    .child(
                        div()
                            .size(px(36.0))
                            .rounded(px(12.0))
                            .bg(accent.opacity(0.12))
                            .border_1()
                            .border_color(accent.opacity(0.24))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                Icon::new(IconName::LayoutDashboard)
                                    .size(px(17.0))
                                    .text_color(accent),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(24.0))
                            .font_semibold()
                            .text_color(theme::fg_primary())
                            .child("System Core"),
                    ),
            )
    }
}
