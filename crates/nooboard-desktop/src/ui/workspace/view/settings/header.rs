use gpui::{Div, ParentElement, Styled, div, px};
use gpui_component::{Icon, IconName, StyledExt};

use crate::ui::theme;

use super::WorkspaceView;

impl WorkspaceView {
    pub(super) fn settings_header(&self) -> Div {
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
                            .size(px(34.0))
                            .rounded(px(12.0))
                            .bg(theme::accent_rose().opacity(0.14))
                            .border_1()
                            .border_color(theme::accent_rose().opacity(0.3))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                Icon::new(IconName::Settings2)
                                    .size(px(16.0))
                                    .text_color(theme::accent_rose()),
                            ),
                    )
                    .child(
                        div()
                            .v_flex()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_size(px(23.0))
                                    .font_semibold()
                                    .text_color(theme::fg_primary())
                                    .child("Settings"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(theme::fg_muted())
                                    .child(
                                        "Storage and network patches (stage5 wireframe layout).",
                                    ),
                            ),
                    ),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .font_semibold()
                    .text_color(theme::fg_muted())
                    .child("NO NEW BACKEND CALLS"),
            )
    }
}
