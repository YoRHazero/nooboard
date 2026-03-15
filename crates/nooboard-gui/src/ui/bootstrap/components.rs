use gpui::{Context, IntoElement, ParentElement, SharedString, Styled, div, px};
use gpui_component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_component::{Disableable, Icon, IconName, StyledExt};

use crate::{
    bootstrap::BootstrapPreset,
    ui::{BootstrapChooserView, theme},
};

const ACTION_BUTTON_WIDTH: f32 = 116.0;
const ACTION_BUTTON_HEIGHT: f32 = 40.0;

impl BootstrapChooserView {
    pub(super) fn preset_button(
        &self,
        id: &'static str,
        preset: BootstrapPreset,
        cx: &Context<Self>,
    ) -> Button {
        let selected = self.controller.view_state().selected_preset == preset;
        let accent = preset_accent(preset);
        let variant = ButtonCustomVariant::new(cx)
            .color(if selected {
                accent.opacity(0.18)
            } else {
                theme::bg_panel_alt()
            })
            .foreground(theme::fg_primary())
            .hover(if selected {
                accent.opacity(0.24)
            } else {
                theme::bg_panel_highlight()
            })
            .active(if selected {
                accent.opacity(0.28)
            } else {
                theme::bg_panel_highlight()
            })
            .shadow(false);

        Button::new(id)
            .custom(variant)
            .disabled(self.controller.view_state().launch_in_flight)
            .w_full()
            .rounded(px(18.0))
            .border_1()
            .border_color(if selected {
                accent.opacity(0.36)
            } else {
                theme::border_soft()
            })
            .p(px(0.0))
            .child(
                div()
                    .w_full()
                    .h(px(54.0))
                    .flex()
                    .items_center()
                    .px(px(16.0))
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_semibold()
                            .text_color(theme::fg_primary())
                            .child(preset.title()),
                    ),
            )
    }

    pub(super) fn action_button(
        &self,
        id: &'static str,
        label: &str,
        accent: gpui::Hsla,
        disabled: bool,
        cx: &Context<Self>,
    ) -> Button {
        let variant = ButtonCustomVariant::new(cx)
            .color(if disabled {
                theme::bg_panel_alt()
            } else {
                accent.opacity(0.92)
            })
            .foreground(if disabled {
                theme::fg_muted()
            } else {
                theme::fg_primary()
            })
            .hover(if disabled {
                theme::bg_panel_alt()
            } else {
                accent
            })
            .active(if disabled {
                theme::bg_panel_alt()
            } else {
                accent.opacity(0.82)
            })
            .shadow(false);

        Button::new(id)
            .custom(variant)
            .disabled(disabled)
            .rounded(px(14.0))
            .border_1()
            .border_color(if disabled {
                theme::border_soft()
            } else {
                accent.opacity(0.30)
            })
            .w(px(ACTION_BUTTON_WIDTH))
            .h(px(ACTION_BUTTON_HEIGHT))
            .child(
                div()
                    .w_full()
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(15.0))
                            .font_semibold()
                            .text_color(if disabled {
                                theme::fg_muted()
                            } else {
                                theme::fg_primary()
                            })
                            .child(label.to_string()),
                    ),
            )
    }

    pub(super) fn action_placeholder(&self) -> impl IntoElement {
        div().w(px(ACTION_BUTTON_WIDTH)).h(px(ACTION_BUTTON_HEIGHT))
    }

    pub(super) fn chooser_title_icon(&self) -> impl IntoElement {
        div()
            .size(px(34.0))
            .rounded(px(12.0))
            .bg(theme::accent_cyan().opacity(0.14))
            .border_1()
            .border_color(theme::accent_cyan().opacity(0.3))
            .flex()
            .items_center()
            .justify_center()
            .child(
                Icon::new(IconName::Settings2)
                    .size(px(16.0))
                    .text_color(theme::accent_cyan()),
            )
    }

    pub(super) fn feedback_banner(&self, message: String) -> impl IntoElement {
        div()
            .w_full()
            .rounded(px(18.0))
            .border_1()
            .border_color(theme::accent_rose().opacity(0.28))
            .bg(theme::accent_rose().opacity(0.10))
            .p(px(14.0))
            .text_size(px(12.0))
            .line_height(px(18.0))
            .text_color(theme::fg_primary())
            .child(SharedString::from(message))
    }
}

fn preset_accent(preset: BootstrapPreset) -> gpui::Hsla {
    match preset {
        BootstrapPreset::DefaultConfig => theme::accent_green(),
        BootstrapPreset::ExistingConfig => theme::accent_cyan(),
        BootstrapPreset::CustomLocation => theme::accent_amber(),
        BootstrapPreset::RepoDevelopment => theme::accent_rose(),
    }
}
