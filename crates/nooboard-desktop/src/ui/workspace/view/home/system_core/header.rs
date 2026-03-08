use super::*;

impl WorkspaceView {
    pub(super) fn system_core_header(&self) -> Div {
        let accent = theme::accent_cyan();

        div()
            .h_flex()
            .items_center()
            .justify_between()
            .gap(px(16.0))
            .child(system_core_title_lockup(accent))
    }
}
