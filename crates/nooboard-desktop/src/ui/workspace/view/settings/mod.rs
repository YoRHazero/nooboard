mod header;
mod network;
mod page_state;
mod storage;

use gpui::{Context, Div, Hsla, ParentElement, Styled, div, px};
use gpui_component::StyledExt;
use gpui_component::button::{Button, ButtonCustomVariant, ButtonVariants};

use crate::ui::theme;

use super::WorkspaceView;

pub(super) use page_state::SettingsPageState;

impl WorkspaceView {
    pub(super) fn settings_page(&self, cx: &mut Context<Self>) -> Div {
        let mut page = div()
            .w_full()
            .v_flex()
            .gap(px(18.0))
            .child(self.settings_header())
            .child(
                div()
                    .h_flex()
                    .items_start()
                    .gap(px(18.0))
                    .child(self.storage_settings_panel(cx))
                    .child(self.network_settings_panel(cx)),
            );

        if let Some(feedback) = self.settings_feedback() {
            page = page.child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(10.0))
                    .px(px(16.0))
                    .py(px(12.0))
                    .bg(theme::bg_panel())
                    .border_1()
                    .border_color(theme::border_base())
                    .rounded(px(16.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_semibold()
                            .text_color(theme::accent_cyan())
                            .child("PATCH QUEUE"),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme::fg_secondary())
                            .child(feedback.to_string()),
                    ),
            );
        }

        page
    }

    fn settings_field_row(&self, label: &'static str, value: String) -> Div {
        div()
            .h_flex()
            .items_center()
            .justify_between()
            .gap(px(12.0))
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::fg_secondary())
                    .child(label.to_string()),
            )
            .child(
                div()
                    .min_w(px(172.0))
                    .max_w(px(320.0))
                    .px(px(10.0))
                    .py(px(6.0))
                    .bg(theme::bg_console())
                    .border_1()
                    .border_color(theme::border_soft())
                    .rounded(px(10.0))
                    .text_size(px(11.0))
                    .text_color(theme::fg_primary())
                    .truncate()
                    .child(value),
            )
    }

    fn settings_action_button(
        &self,
        id: impl Into<gpui::ElementId>,
        label: &str,
        accent: Hsla,
        cx: &mut Context<Self>,
    ) -> Button {
        let variant = ButtonCustomVariant::new(cx)
            .color(accent.opacity(0.12))
            .foreground(theme::fg_primary())
            .hover(accent.opacity(0.2))
            .active(accent.opacity(0.28))
            .shadow(false);

        Button::new(id)
            .custom(variant)
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
