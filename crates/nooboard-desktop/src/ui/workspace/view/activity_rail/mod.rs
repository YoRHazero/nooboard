mod panels;

use gpui::{Div, ParentElement, Styled, div, px};
use gpui_component::StyledExt;
use gpui_component::scroll::ScrollableElement;

use crate::ui::theme;

use super::{
    WorkspaceView,
    components::{console_pill, pulse_beacon},
    shared::ACTIVITY_WIDTH,
};

impl WorkspaceView {
    pub(super) fn activity_rail(&self) -> Div {
        div()
            .w(px(ACTIVITY_WIDTH))
            .h_full()
            .min_h_0()
            .bg(theme::bg_activity())
            .border_1()
            .border_color(theme::border_base())
            .rounded(px(26.0))
            .shadow_xs()
            .child(
                div()
                    .v_flex()
                    .h_full()
                    .gap(px(18.0))
                    .p(px(16.0))
                    .child(
                        div()
                            .v_flex()
                            .gap(px(12.0))
                            .p(px(14.0))
                            .bg(theme::bg_rail_panel())
                            .border_1()
                            .border_color(theme::border_soft())
                            .rounded(px(22.0))
                            .child(
                                div()
                                    .h_flex()
                                    .items_center()
                                    .gap(px(10.0))
                                    .child(pulse_beacon("activity-rail-beacon", theme::accent_cyan()))
                                    .child(
                                        div()
                                            .text_size(px(16.0))
                                            .font_semibold()
                                            .text_color(theme::fg_primary())
                                            .child("Operations Feed"),
                                    ),
                            )
                            .child(
                                div()
                                    .h_flex()
                                    .gap(px(8.0))
                                    .items_center()
                                    .child(console_pill("telemetry", theme::accent_cyan()))
                                    .child(console_pill("review", theme::accent_amber()))
                                    .child(console_pill("notes", theme::accent_rose())),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(theme::fg_secondary())
                                    .line_clamp(2)
                                    .text_ellipsis()
                                    .child("A side telemetry column with tighter framing and darker panel treatment, intentionally distinct from the main canvas surfaces."),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scrollbar()
                            .child(
                                div()
                                    .v_flex()
                                    .gap(px(20.0))
                                    .child(self.activity_panel())
                                    .child(self.pending_files_panel())
                                    .child(self.error_panel()),
                            ),
                    ),
            )
    }
}
