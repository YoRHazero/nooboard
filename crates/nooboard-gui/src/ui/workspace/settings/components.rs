use gpui::{
    AnyElement, AnyView, App, Context, InteractiveElement, IntoElement, ParentElement, Pixels,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder as _, px,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::tooltip::Tooltip;
use gpui_component::{IconName, Sizable, StyledExt};

use crate::ui::theme;

use super::super::WorkspaceView;

impl WorkspaceView {
    pub(super) fn settings_themed_tooltip(
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

    pub(super) fn settings_feedback_banner(
        &self,
        label: &str,
        accent: gpui::Hsla,
        message: impl Into<String>,
    ) -> gpui::Div {
        div()
            .min_h(px(56.0))
            .h_flex()
            .items_center()
            .justify_between()
            .gap(px(14.0))
            .px(px(16.0))
            .py(px(12.0))
            .bg(theme::bg_panel())
            .border_1()
            .border_color(accent.opacity(0.28))
            .rounded(px(18.0))
            .shadow_xs()
            .child(self.settings_status_chip(label, accent))
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

    pub(super) fn settings_section_shell(
        &self,
        title: &str,
        description: &str,
        status: impl IntoElement,
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
                                    .child(description.to_string()),
                            ),
                    )
                    .child(status),
            )
            .child(div().h(px(1.0)).w_full().bg(theme::border_soft()))
    }

    pub(super) fn settings_status_chip(&self, label: &str, accent: gpui::Hsla) -> gpui::Div {
        div()
            .h_flex()
            .items_center()
            .justify_center()
            .gap(px(8.0))
            .px(px(10.0))
            .py(px(6.0))
            .rounded(px(999.0))
            .bg(accent.opacity(0.12))
            .border_1()
            .border_color(accent.opacity(0.26))
            .child(div().size(px(6.0)).rounded(px(999.0)).bg(accent))
            .child(
                div()
                    .text_size(px(10.0))
                    .font_semibold()
                    .text_color(accent)
                    .child(label.to_string()),
            )
    }

    pub(super) fn settings_input_field_with_tooltip(
        &self,
        id_prefix: &str,
        label: &str,
        tooltip: &str,
        input: impl IntoElement,
    ) -> impl IntoElement {
        div()
            .min_w(px(212.0))
            .flex_1()
            .v_flex()
            .gap(px(6.0))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme::fg_secondary())
                            .child(label.to_string()),
                    )
                    .child(self.settings_info_tooltip_icon(
                        id_prefix,
                        tooltip.to_string(),
                    )),
            )
            .child(
                div()
                    .child(self.settings_input_surface(input)),
            )
    }

    pub(super) fn settings_fixed_field_with_tooltip(
        &self,
        id_prefix: &str,
        label: &str,
        tooltip: &str,
        width: Pixels,
        content: impl IntoElement,
    ) -> impl IntoElement {
        div()
            .w(width)
            .flex_none()
            .v_flex()
            .gap(px(6.0))
            .items_start()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme::fg_secondary())
                            .child(label.to_string()),
                    )
                    .child(self.settings_info_tooltip_icon(
                        id_prefix,
                        tooltip.to_string(),
                    )),
            )
            .child(content)
    }

    pub(super) fn settings_input_surface(&self, content: impl IntoElement) -> impl IntoElement {
        div()
            .px(px(12.0))
            .py(px(10.0))
            .bg(theme::bg_console())
            .border_1()
            .border_color(theme::border_soft())
            .rounded(px(16.0))
            .child(content)
    }

    pub(super) fn settings_info_tooltip_icon(
        &self,
        id_prefix: &str,
        tooltip: String,
    ) -> impl IntoElement {
        let shell_id = format!("{id_prefix}-tooltip-shell");
        let button_id = format!("{id_prefix}-tooltip-button");

        div()
            .id(shell_id)
            .tooltip(move |window, cx| Self::settings_themed_tooltip(tooltip.clone(), window, cx))
            .child(
                Button::new(button_id)
                    .ghost()
                    .xsmall()
                    .icon(IconName::Info),
            )
    }

    pub(super) fn settings_inline_switch(
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

    pub(super) fn settings_readonly_path_card(&self, label: &str, value: String) -> gpui::Div {
        self.settings_readonly_value_card(label, value)
    }

    pub(super) fn settings_readonly_value_card(&self, label: &str, value: String) -> gpui::Div {
        self.settings_readonly_value_card_with_action(label, value, None)
    }

    pub(super) fn settings_readonly_value_card_with_action(
        &self,
        label: &str,
        value: String,
        action: Option<AnyElement>,
    ) -> gpui::Div {
        div()
            .v_flex()
            .gap(px(8.0))
            .p(px(14.0))
            .bg(theme::bg_console())
            .border_1()
            .border_color(theme::border_soft())
            .rounded(px(18.0))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_semibold()
                            .text_color(theme::fg_primary())
                            .child(label.to_string()),
                    )
                    .child(self.settings_status_chip("Read-only", theme::accent_amber())),
            )
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(10.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .text_size(px(11.0))
                            .text_color(theme::fg_secondary())
                            .line_clamp(2)
                            .text_ellipsis()
                            .child(value),
                    )
                    .when_some(action, |this, action| {
                        this.child(div().flex_none().child(action))
                    }),
            )
    }

    pub(super) fn settings_compact_action_button(
        &self,
        id: impl Into<gpui::ElementId>,
        label: &str,
        tooltip: String,
        enabled: bool,
        accent: gpui::Hsla,
        on_click: impl Fn(&mut Self, &gpui::ClickEvent, &mut Window, &mut Context<Self>) + 'static,
        cx: &Context<Self>,
    ) -> AnyElement {
        let button = div()
            .id(id)
            .min_w(px(76.0))
            .h(px(34.0))
            .px(px(12.0))
            .h_flex()
            .items_center()
            .justify_center()
            .rounded(px(12.0))
            .bg(theme::bg_console())
            .border_1()
            .border_color(if enabled {
                accent.opacity(0.28)
            } else {
                theme::border_soft()
            })
            .tooltip(move |window, cx| {
                Self::settings_themed_tooltip(tooltip.clone(), window, cx)
            })
            .child(
                div()
                    .text_size(px(11.0))
                    .font_semibold()
                    .text_color(if enabled {
                        theme::fg_primary()
                    } else {
                        theme::fg_muted()
                    })
                    .child(label.to_string()),
            );

        if enabled {
            button
                .cursor_pointer()
                .hover(move |this| {
                    this.bg(accent.opacity(0.16))
                        .border_color(accent.opacity(0.54))
                })
                .active(move |this| {
                    this.bg(accent.opacity(0.22))
                        .border_color(accent.opacity(0.70))
                })
                .on_click(cx.listener(on_click))
                .into_any_element()
        } else {
            button.opacity(0.52).into_any_element()
        }
    }
}
