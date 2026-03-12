use gpui::{ClipboardItem, Context, Div, IntoElement, ParentElement, Styled, div, px};
use gpui_component::StyledExt;

use crate::ui::theme;

use super::WorkspaceView;
use super::components::{
    settings_action_button, settings_action_row, settings_section_footer, settings_section_shell,
    settings_status_chip,
};

impl WorkspaceView {
    pub(super) fn advanced_settings_panel(&self, cx: &mut Context<Self>) -> Div {
        let status = settings_status_chip("Current", theme::accent_green());
        let config_path = self.advanced_settings().config_path.display().to_string();

        settings_section_shell(
            "Advanced",
            "Inspect the active bootstrap mode and configuration file currently driving the app.",
            status,
        )
        .child(settings_info_row(
            "Bootstrap mode",
            self.advanced_bootstrap_mode_label(),
            false,
        ))
        .child(settings_info_row("Config path", config_path, true))
        .child(settings_section_footer(
            "This section is read-only. Change bootstrap behavior by restarting with a different bootstrap choice or CLI override.",
            theme::fg_muted(),
            settings_action_row([
                settings_action_button(
                    "settings-copy-config-path",
                    "Copy Path",
                    theme::accent_cyan(),
                    cx,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.copy_settings_config_path(cx);
                }))
                .into_any_element(),
            ]),
        ))
    }

    pub(super) fn copy_settings_config_path(&mut self, cx: &mut Context<Self>) {
        let config_path = self.advanced_settings().config_path.display().to_string();
        cx.write_to_clipboard(ClipboardItem::new_string(config_path));
        self.set_settings_feedback("Copied config path to clipboard.");
        cx.notify();
    }
}

fn settings_info_row(label: &str, value: impl Into<String>, multiline: bool) -> Div {
    let value = value.into();

    div()
        .v_flex()
        .gap(px(8.0))
        .child(
            div()
                .text_size(px(11.0))
                .text_color(theme::fg_secondary())
                .child(label.to_string()),
        )
        .child(
            div()
                .w_full()
                .min_h(px(42.0))
                .px(px(12.0))
                .py(px(10.0))
                .bg(theme::bg_console())
                .border_1()
                .border_color(theme::border_soft())
                .rounded(px(14.0))
                .child(
                    div()
                        .w_full()
                        .min_w(px(0.0))
                        .text_size(px(12.0))
                        .text_color(theme::fg_primary())
                        .line_clamp(if multiline { 2 } else { 1 })
                        .text_ellipsis()
                        .child(value),
                ),
        )
}
