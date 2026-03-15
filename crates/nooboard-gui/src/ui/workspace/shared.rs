use gpui::{
    Context, Div, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, Window, div, px,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{Disableable, StyledExt};

use crate::{
    ui::theme,
    workspace::{route::WorkspaceRoute, view_state::WorkspaceMetricViewState},
};

use super::WorkspaceView;

impl WorkspaceView {
    pub(super) fn current_route(&self, cx: &Context<Self>) -> WorkspaceRoute {
        self.controller.read(cx).route()
    }

    pub(super) fn nav_item(
        &self,
        id: &'static str,
        label: &'static str,
        route: WorkspaceRoute,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active = self.current_route(cx) == route;

        div()
            .id(id)
            .cursor_pointer()
            .px(px(14.0))
            .py(px(12.0))
            .rounded(px(18.0))
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
            .on_click(cx.listener(move |this, _, _, cx| {
                let _ = this.controller.update(cx, |controller, cx| {
                    controller.set_route(route);
                    cx.notify();
                });
            }))
            .child(
                div()
                    .text_size(px(13.0))
                    .font_semibold()
                    .text_color(if active {
                        theme::fg_primary()
                    } else {
                        theme::fg_secondary()
                    })
                    .child(label),
            )
    }

    pub(super) fn titlebar_chip(&self, label: &str, value: String, accent: gpui::Hsla) -> Div {
        div()
            .h_flex()
            .h(px(22.0))
            .gap(px(8.0))
            .items_center()
            .px(px(8.0))
            .bg(theme::bg_panel_alt())
            .rounded(px(999.0))
            .child(div().size(px(6.0)).rounded(px(999.0)).bg(accent))
            .child(
                div()
                    .text_size(px(10.0))
                    .font_semibold()
                    .text_color(theme::fg_secondary())
                    .child(label.to_uppercase()),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .font_semibold()
                    .text_color(theme::fg_primary())
                    .child(value),
            )
    }

    pub(super) fn metric_card(&self, metric: &WorkspaceMetricViewState) -> Div {
        div()
            .v_flex()
            .gap(px(10.0))
            .p(px(16.0))
            .bg(theme::bg_panel())
            .border_1()
            .border_color(theme::border_soft())
            .rounded(px(20.0))
            .min_w(px(150.0))
            .child(
                div()
                    .text_size(px(11.0))
                    .font_semibold()
                    .text_color(theme::fg_muted())
                    .child(metric.label),
            )
            .child(
                div()
                    .text_size(px(18.0))
                    .font_semibold()
                    .text_color(theme::fg_primary())
                    .child(metric.value.clone()),
            )
    }

    pub(super) fn stream_badge(&self, label: &str, open: bool) -> Div {
        let accent = if open {
            theme::accent_green()
        } else {
            theme::accent_rose()
        };

        div()
            .h_flex()
            .items_center()
            .gap(px(8.0))
            .px(px(10.0))
            .py(px(6.0))
            .rounded(px(999.0))
            .bg(theme::bg_panel())
            .border_1()
            .border_color(accent.opacity(0.24))
            .child(div().size(px(7.0)).rounded(px(999.0)).bg(accent))
            .child(
                div()
                    .text_size(px(11.0))
                    .font_semibold()
                    .text_color(theme::fg_secondary())
                    .child(format!("{label}: {}", if open { "open" } else { "closed" })),
            )
    }

    pub(super) fn toolbar_button(
        &self,
        id: &'static str,
        label: &str,
        enabled: bool,
        accent: gpui::Hsla,
        on_click: impl Fn(&mut Self, &gpui::ClickEvent, &mut Window, &mut Context<Self>) + 'static,
        cx: &Context<Self>,
    ) -> Button {
        Button::new(id)
            .primary()
            .disabled(!enabled)
            .border_1()
            .border_color(accent.opacity(0.24))
            .on_click(cx.listener(on_click))
            .child(
                div()
                    .px(px(14.0))
                    .py(px(8.0))
                    .text_size(px(12.0))
                    .font_semibold()
                    .child(label.to_string()),
            )
    }

    pub(super) fn list_card(&self, title: &str, items: &[String], empty: &str) -> Div {
        div()
            .v_flex()
            .gap(px(10.0))
            .p(px(18.0))
            .bg(theme::bg_panel())
            .border_1()
            .border_color(theme::border_soft())
            .rounded(px(22.0))
            .child(
                div()
                    .text_size(px(12.0))
                    .font_semibold()
                    .text_color(theme::fg_secondary())
                    .child(title.to_string()),
            )
            .children(if items.is_empty() {
                vec![
                    div()
                        .text_size(px(12.0))
                        .text_color(theme::fg_muted())
                        .child(empty.to_string())
                        .into_any_element(),
                ]
            } else {
                items
                    .iter()
                    .map(|item| {
                        div()
                            .text_size(px(12.0))
                            .line_height(px(18.0))
                            .text_color(theme::fg_primary())
                            .child(item.clone())
                            .into_any_element()
                    })
                    .collect()
            })
    }
}
