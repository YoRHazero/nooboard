use gpui::{Div, ParentElement, Styled, div, px};
use gpui_component::{Icon, IconName, StyledExt};

use crate::ui::theme;

use super::WorkspaceView;
use super::components::settings_status_chip;

impl WorkspaceView {
    pub(super) fn settings_header(&self) -> Div {
        let dirty_fields = self.settings_dirty_field_count();
        let validation_issues = self.storage_validation_issues();
        let (status_label, status_accent) = if !validation_issues.is_empty() {
            ("Review", theme::accent_rose())
        } else if dirty_fields > 0 {
            ("Modified", theme::accent_amber())
        } else {
            ("Current", theme::accent_green())
        };

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
                    .flex_1()
                    .min_w(px(0.0))
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
                            .flex_1()
                            .min_w(px(0.0))
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
                                    .line_clamp(1)
                                    .text_ellipsis()
                                    .child("Review draft changes against the current settings."),
                            ),
                    ),
            )
            .child(
                div()
                    .w(px(208.0))
                    .h_flex()
                    .items_center()
                    .justify_end()
                    .gap(px(8.0))
                    .child(settings_status_chip(status_label, status_accent))
                    .child(settings_status_chip(
                        format!("{} dirty", dirty_fields),
                        theme::accent_cyan(),
                    )),
            )
    }
}
