use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyView, App, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_component::tooltip::Tooltip;
use gpui_component::{Sizable, StyledExt};

use crate::ui::theme;

use super::super::WorkspaceView;

impl WorkspaceView {
    pub(in crate::ui::workspace::network) fn network_themed_tooltip(
        text: String,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyView {
        Tooltip::new(text)
            .bg(theme::bg_panel())
            .text_color(theme::fg_primary())
            .border_color(theme::border_base())
            .build(window, cx)
    }

    pub(in crate::ui::workspace::network) fn network_feedback_banner(
        &self,
        accent: gpui::Hsla,
        message: impl Into<String>,
    ) -> gpui::Div {
        div()
            .min_h(px(52.0))
            .h_flex()
            .items_center()
            .gap(px(12.0))
            .px(px(16.0))
            .py(px(12.0))
            .bg(theme::bg_panel())
            .border_1()
            .border_color(accent.opacity(0.26))
            .rounded(px(18.0))
            .shadow_xs()
            .child(div().size(px(8.0)).rounded(px(999.0)).bg(accent))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .text_size(px(12.0))
                    .text_color(theme::fg_secondary())
                    .line_clamp(2)
                    .text_ellipsis()
                    .child(message.into()),
            )
    }

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
                    .items_start()
                    .justify_between()
                    .gap(px(12.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .v_flex()
                            .gap(px(6.0))
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
                                    .line_clamp(2)
                                    .text_ellipsis()
                                    .child(detail.into()),
                            ),
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

    pub(in crate::ui::workspace::network) fn network_metric_chip(
        &self,
        label: &str,
        value: impl Into<String>,
        accent: gpui::Hsla,
    ) -> gpui::Div {
        div()
            .h_flex()
            .items_center()
            .gap(px(8.0))
            .px(px(10.0))
            .py(px(7.0))
            .rounded(px(16.0))
            .bg(accent.opacity(0.10))
            .border_1()
            .border_color(accent.opacity(0.22))
            .child(
                div()
                    .text_size(px(10.0))
                    .font_semibold()
                    .text_color(accent)
                    .child(label.to_uppercase()),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::fg_secondary())
                    .child(value.into()),
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

    pub(in crate::ui::workspace::network) fn network_meta_pill(
        &self,
        label: &str,
        value: &str,
        accent: gpui::Hsla,
    ) -> gpui::Div {
        div()
            .v_flex()
            .gap(px(4.0))
            .min_w(px(140.0))
            .px(px(12.0))
            .py(px(10.0))
            .bg(theme::bg_console())
            .border_1()
            .border_color(theme::border_soft())
            .rounded(px(16.0))
            .child(
                div()
                    .text_size(px(10.0))
                    .font_semibold()
                    .text_color(accent)
                    .child(label.to_uppercase()),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(theme::fg_primary())
                    .line_clamp(2)
                    .text_ellipsis()
                    .child(value.to_string()),
            )
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

    pub(in crate::ui::workspace::network) fn network_toggle_switch(
        &self,
        id: impl Into<gpui::ElementId>,
        label: &str,
        enabled: bool,
        accent: gpui::Hsla,
        detail: &str,
        on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        div()
            .id(id)
            .cursor_pointer()
            .min_w(px(220.0))
            .px(px(14.0))
            .py(px(12.0))
            .bg(theme::bg_console())
            .border_1()
            .border_color(if enabled {
                accent.opacity(0.34)
            } else {
                theme::border_soft()
            })
            .rounded(px(18.0))
            .hover(|this| this.bg(theme::bg_panel_alt()))
            .on_click(on_click)
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.0))
                    .child(
                        div()
                            .v_flex()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_semibold()
                                    .text_color(theme::fg_primary())
                                    .child(label.to_string()),
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(theme::fg_muted())
                                    .line_clamp(2)
                                    .text_ellipsis()
                                    .child(detail.to_string()),
                            ),
                    )
                    .child(
                        div()
                            .relative()
                            .w(px(42.0))
                            .h(px(24.0))
                            .rounded(px(999.0))
                            .bg(if enabled {
                                accent.opacity(0.24)
                            } else {
                                theme::border_soft()
                            })
                            .child(
                                div()
                                    .absolute()
                                    .top(px(3.0))
                                    .left(if enabled { px(21.0) } else { px(3.0) })
                                    .size(px(18.0))
                                    .rounded(px(999.0))
                                    .bg(if enabled {
                                        accent
                                    } else {
                                        theme::fg_secondary()
                                    }),
                            ),
                    ),
            )
    }

    pub(in crate::ui::workspace::network) fn network_inline_switch(
        &self,
        id: impl Into<gpui::ElementId>,
        enabled: bool,
        accent: gpui::Hsla,
        on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        div()
            .id(id)
            .cursor_pointer()
            .relative()
            .w(px(46.0))
            .h(px(26.0))
            .rounded(px(999.0))
            .bg(if enabled {
                accent.opacity(0.22)
            } else {
                theme::border_soft()
            })
            .border_1()
            .border_color(if enabled {
                accent.opacity(0.34)
            } else {
                theme::border_base()
            })
            .hover(|this| this.bg(theme::bg_panel_alt()))
            .on_click(on_click)
            .child(
                div()
                    .absolute()
                    .top(px(3.0))
                    .left(if enabled { px(23.0) } else { px(3.0) })
                    .size(px(18.0))
                    .rounded(px(999.0))
                    .bg(if enabled {
                        accent
                    } else {
                        theme::fg_secondary()
                    }),
            )
    }

    pub(in crate::ui::workspace::network) fn network_segment_button(
        &self,
        id: impl Into<gpui::ElementId>,
        label: &str,
        active: bool,
        on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        div()
            .id(id)
            .cursor_pointer()
            .px(px(12.0))
            .py(px(8.0))
            .rounded(px(14.0))
            .bg(if active {
                theme::bg_panel_highlight()
            } else {
                theme::bg_console()
            })
            .border_1()
            .border_color(if active {
                theme::border_strong()
            } else {
                theme::border_soft()
            })
            .hover(|this| this.bg(theme::bg_panel_alt()))
            .on_click(on_click)
            .child(
                div()
                    .text_size(px(11.0))
                    .font_semibold()
                    .text_color(if active {
                        theme::fg_primary()
                    } else {
                        theme::fg_secondary()
                    })
                    .child(label.to_string()),
            )
    }
}
