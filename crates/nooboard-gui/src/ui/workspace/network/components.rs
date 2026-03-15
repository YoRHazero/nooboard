use gpui::prelude::FluentBuilder as _;
use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    div, px,
};
use gpui_component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_component::{Sizable, StyledExt};

use crate::ui::theme;

use super::super::WorkspaceView;

impl WorkspaceView {
    pub(in crate::ui::workspace::network) fn network_panel_shell(
        &self,
        title: &str,
        detail: impl Into<String>,
    ) -> gpui::Div {
        div()
            .v_flex()
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
                    .justify_between()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(18.0))
                            .font_semibold()
                            .text_color(theme::fg_primary())
                            .child(title.to_string()),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme::fg_muted())
                            .child(detail.into()),
                    ),
            )
    }

    pub(in crate::ui::workspace::network) fn network_row_shell(&self) -> gpui::Div {
        div()
            .v_flex()
            .gap(px(10.0))
            .p(px(14.0))
            .bg(theme::bg_console())
            .border_1()
            .border_color(theme::border_soft())
            .rounded(px(18.0))
    }

    pub(in crate::ui::workspace::network) fn network_empty_notice(&self, label: &str) -> gpui::Div {
        div()
            .p(px(12.0))
            .bg(theme::bg_activity())
            .border_1()
            .border_color(theme::border_soft())
            .rounded(px(16.0))
            .text_size(px(12.0))
            .text_color(theme::fg_muted())
            .child(label.to_string())
    }

    pub(in crate::ui::workspace::network) fn network_metric_card(
        &self,
        label: &str,
        value: usize,
        accent: gpui::Hsla,
    ) -> gpui::Div {
        div()
            .min_w(px(152.0))
            .v_flex()
            .gap(px(10.0))
            .p(px(14.0))
            .bg(theme::bg_panel())
            .border_1()
            .border_color(theme::border_base())
            .rounded(px(20.0))
            .shadow_xs()
            .child(
                div()
                    .text_size(px(11.0))
                    .font_semibold()
                    .text_color(theme::fg_secondary())
                    .child(label.to_string()),
            )
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(30.0))
                            .font_semibold()
                            .text_color(theme::fg_primary())
                            .child(value.to_string()),
                    )
                    .child(div().size(px(8.0)).rounded(px(999.0)).bg(accent)),
            )
    }

    pub(in crate::ui::workspace::network) fn network_status_chip(
        &self,
        label: &str,
        detail: &str,
        accent: gpui::Hsla,
    ) -> gpui::Div {
        div()
            .h_flex()
            .items_center()
            .gap(px(8.0))
            .px(px(12.0))
            .py(px(8.0))
            .rounded(px(16.0))
            .bg(accent.opacity(0.12))
            .border_1()
            .border_color(accent.opacity(0.24))
            .child(div().size(px(6.0)).rounded(px(999.0)).bg(accent))
            .child(
                div()
                    .text_size(px(11.0))
                    .font_semibold()
                    .text_color(accent)
                    .child(label.to_string()),
            )
            .when(!detail.is_empty(), |this| {
                this.child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme::fg_secondary())
                        .child(detail.to_string()),
                )
            })
    }

    pub(in crate::ui::workspace::network) fn network_action_button(
        &self,
        id: impl Into<gpui::ElementId>,
        label: &str,
        accent: gpui::Hsla,
        cx: &Context<Self>,
    ) -> Button {
        let variant = ButtonCustomVariant::new(cx)
            .color(accent.opacity(0.12))
            .foreground(theme::fg_primary())
            .hover(accent.opacity(0.2))
            .active(accent.opacity(0.28))
            .shadow(false);

        Button::new(id)
            .custom(variant)
            .small()
            .compact()
            .rounded(px(999.0))
            .border_1()
            .border_color(accent.opacity(0.24))
            .child(
                div()
                    .text_color(theme::fg_primary())
                    .font_semibold()
                    .child(label.to_string()),
            )
    }

    pub(in crate::ui::workspace::network) fn network_input_field(
        &self,
        label: &str,
        input: impl IntoElement,
    ) -> impl IntoElement {
        div()
            .min_w(px(180.0))
            .flex_1()
            .v_flex()
            .gap(px(6.0))
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::fg_secondary())
                    .child(label.to_string()),
            )
            .child(
                div()
                    .px(px(12.0))
                    .py(px(10.0))
                    .bg(theme::bg_console())
                    .border_1()
                    .border_color(theme::border_soft())
                    .rounded(px(16.0))
                    .child(input),
            )
    }

    pub(in crate::ui::workspace::network) fn network_toggle_chip(
        &self,
        enabled: bool,
        label: &str,
        accent: gpui::Hsla,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("network-seed-enabled-toggle")
            .cursor_pointer()
            .h(px(44.0))
            .px(px(12.0))
            .bg(if enabled {
                theme::bg_panel_highlight()
            } else {
                theme::bg_console()
            })
            .border_1()
            .border_color(if enabled {
                accent.opacity(0.34)
            } else {
                theme::border_soft()
            })
            .rounded(px(16.0))
            .h_flex()
            .items_center()
            .gap(px(8.0))
            .child(div().size(px(7.0)).rounded(px(999.0)).bg(if enabled {
                accent
            } else {
                theme::border_base()
            }))
            .child(
                div()
                    .text_size(px(11.0))
                    .font_semibold()
                    .text_color(if enabled {
                        accent
                    } else {
                        theme::fg_secondary()
                    })
                    .child(label.to_string()),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.request_network_toggle_seed_enabled(cx);
            }))
    }
}
