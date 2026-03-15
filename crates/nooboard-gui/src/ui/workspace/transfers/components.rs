use gpui::{Context, IntoElement, ParentElement, Styled, div, px};
use gpui_component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_component::{Sizable, StyledExt};

use crate::ui::theme;

use super::super::WorkspaceView;

impl WorkspaceView {
    pub(in crate::ui::workspace::transfers) fn transfers_panel_shell(
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

    pub(in crate::ui::workspace::transfers) fn transfers_empty_notice(
        &self,
        label: &str,
    ) -> gpui::Div {
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

    pub(in crate::ui::workspace::transfers) fn transfer_section(
        &self,
        title: &str,
        count: usize,
        cards: Vec<gpui::AnyElement>,
        empty_label: &str,
    ) -> gpui::Div {
        div()
            .v_flex()
            .gap(px(10.0))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_semibold()
                            .text_color(theme::fg_primary())
                            .child(title.to_string()),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme::fg_muted())
                            .child(count.to_string()),
                    ),
            )
            .children(if cards.is_empty() {
                vec![self.transfers_empty_notice(empty_label).into_any_element()]
            } else {
                cards
            })
    }

    pub(in crate::ui::workspace::transfers) fn transfers_card_shell(&self) -> gpui::Div {
        div()
            .v_flex()
            .gap(px(10.0))
            .p(px(14.0))
            .bg(theme::bg_rail_panel())
            .border_1()
            .border_color(theme::border_soft())
            .rounded(px(18.0))
    }

    pub(in crate::ui::workspace::transfers) fn transfer_status_badge(
        &self,
        label: &str,
        accent: gpui::Hsla,
    ) -> gpui::Div {
        div()
            .px(px(10.0))
            .py(px(6.0))
            .rounded(px(999.0))
            .bg(accent.opacity(0.12))
            .border_1()
            .border_color(accent.opacity(0.24))
            .text_size(px(10.0))
            .font_semibold()
            .text_color(accent)
            .child(label.to_string())
    }

    pub(in crate::ui::workspace::transfers) fn transfer_card_heading(
        &self,
        file_name: &str,
        accent: gpui::Hsla,
    ) -> gpui::Div {
        div()
            .v_flex()
            .gap(px(8.0))
            .child(div().h(px(2.0)).w_full().bg(accent).rounded(px(999.0)))
            .child(
                div()
                    .text_size(px(13.0))
                    .font_semibold()
                    .text_color(theme::fg_primary())
                    .line_clamp(2)
                    .text_ellipsis()
                    .child(file_name.to_string()),
            )
    }

    pub(in crate::ui::workspace::transfers) fn transfer_card_meta(
        &self,
        source_device: &str,
        size_label: &str,
    ) -> gpui::Div {
        div()
            .text_size(px(11.0))
            .text_color(theme::fg_muted())
            .truncate()
            .child(format!("{} · {}", source_device, size_label))
    }

    pub(in crate::ui::workspace::transfers) fn transfer_action_button(
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
}
