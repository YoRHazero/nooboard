use super::super::snapshot::HomeSystemCoreSnapshot;
use super::*;

impl WorkspaceView {
    pub(super) fn system_core_header(&self, snapshot: &HomeSystemCoreSnapshot) -> Div {
        let accent = theme::accent_cyan();

        div()
            .h_flex()
            .items_center()
            .justify_between()
            .gap(px(16.0))
            .child(system_core_title_lockup(accent))
            .child(
                div()
                    .px(px(12.0))
                    .py(px(6.0))
                    .bg(theme::bg_console())
                    .border_1()
                    .border_color(theme::border_soft())
                    .rounded(px(999.0))
                    .text_size(px(11.0))
                    .font_semibold()
                    .text_color(theme::fg_muted())
                    .child(format!("DEVICE {}", snapshot.local_device_id)),
            )
    }
}
