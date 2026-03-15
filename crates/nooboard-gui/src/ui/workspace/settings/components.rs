use gpui::{
    App, Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, Window, div, px,
};
use gpui_component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_component::{Sizable, StyledExt};

use crate::{
    ui::theme,
    workspace::view_state::{SettingsStorageViewState, SettingsTransfersViewState},
};

use super::super::WorkspaceView;

impl WorkspaceView {
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

    pub(super) fn settings_meta_chip(
        &self,
        label: &str,
        value: &str,
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
                    .child(value.to_string()),
            )
    }

    pub(super) fn settings_input_field(
        &self,
        label: &str,
        hint: &str,
        input: impl IntoElement,
    ) -> impl IntoElement {
        div()
            .min_w(px(212.0))
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
                    .text_size(px(10.0))
                    .text_color(theme::fg_muted())
                    .line_clamp(2)
                    .text_ellipsis()
                    .child(hint.to_string()),
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

    pub(super) fn settings_toggle_chip(
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
            .min_w(px(228.0))
            .flex_1()
            .v_flex()
            .gap(px(8.0))
            .p(px(14.0))
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
            .rounded(px(18.0))
            .hover(|this| this.bg(theme::bg_panel_alt()))
            .on_click(on_click)
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_semibold()
                            .text_color(theme::fg_primary())
                            .child(label.to_string()),
                    )
                    .child(self.settings_status_chip(
                        if enabled { "Enabled" } else { "Disabled" },
                        if enabled {
                            accent
                        } else {
                            theme::accent_amber()
                        },
                    )),
            )
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(theme::fg_muted())
                    .line_clamp(2)
                    .text_ellipsis()
                    .child(detail.to_string()),
            )
    }

    pub(super) fn settings_path_summary(&self, state: &SettingsTransfersViewState) -> gpui::Div {
        div()
            .text_size(px(11.0))
            .text_color(theme::fg_muted())
            .line_clamp(2)
            .text_ellipsis()
            .child(format!(
                "Current download directory: {}",
                state.download_dir.display()
            ))
    }

    pub(super) fn settings_storage_summary(&self, state: &SettingsStorageViewState) -> gpui::Div {
        div()
            .text_size(px(11.0))
            .text_color(theme::fg_muted())
            .line_clamp(2)
            .text_ellipsis()
            .child(format!(
                "Current storage window: {}d history · {}d dedup · {} bytes max text · GC batch {}",
                state.history_window_days,
                state.dedup_window_days,
                state.max_text_bytes,
                state.gc_batch_size
            ))
    }

    pub(super) fn settings_readonly_path_card(&self, label: &str, value: String) -> gpui::Div {
        self.settings_readonly_value_card(label, value)
    }

    pub(super) fn settings_readonly_value_card(&self, label: &str, value: String) -> gpui::Div {
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
                    .text_size(px(11.0))
                    .text_color(theme::fg_secondary())
                    .line_clamp(2)
                    .text_ellipsis()
                    .child(value),
            )
    }

    pub(super) fn settings_action_button(
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
            .rounded(px(12.0))
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
