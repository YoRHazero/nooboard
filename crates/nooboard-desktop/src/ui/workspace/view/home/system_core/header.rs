use super::*;

impl WorkspaceView {
    pub(super) fn system_core_header(&self) -> Div {
        let sync_label = self.sync_label();
        let desired_label = self.desired_state_label();
        let status_accent = self.system_core_status_accent();

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
                            .bg(status_accent.opacity(0.12))
                            .border_1()
                            .border_color(status_accent.opacity(0.24))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                Icon::new(IconName::LayoutDashboard)
                                    .size(px(17.0))
                                    .text_color(status_accent),
                            ),
                    )
                    .child(
                        div()
                            .v_flex()
                            .gap(px(3.0))
                            .child(
                                div()
                                    .text_size(px(24.0))
                                    .font_semibold()
                                    .text_color(theme::fg_primary())
                                    .child("System Core"),
                            )
                            .child(
                                div()
                                    .h_flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(div().size(px(7.0)).rounded(px(999.0)).bg(status_accent))
                                    .child(
                                        div()
                                            .text_size(px(10.0))
                                            .font_semibold()
                                            .text_color(theme::fg_secondary())
                                            .child(sync_label),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.0))
                                            .text_color(theme::fg_muted())
                                            .child(format!("target {desired_label}")),
                                    ),
                            ),
                    ),
            )
    }
}
