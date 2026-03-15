use gpui::{Context, IntoElement, ParentElement, Styled, div};
use gpui_component::StyledExt;

use crate::ui::theme;

use super::super::super::{WorkspaceRenderModel, WorkspaceView};

impl WorkspaceView {
    pub(in crate::ui::workspace::settings) fn advanced_settings_panel(
        &self,
        model: &WorkspaceRenderModel,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let config_path = model.shell.config_path_label.clone();

        self.settings_section_shell(
            "Advanced",
            "Inspect the active bootstrap mode and configuration file currently driving the app.",
            self.settings_status_chip("Current", theme::accent_green()),
        )
        .child(
            div()
                .h_flex()
                .flex_wrap()
                .gap(gpui::px(12.0))
                .child(self.settings_readonly_value_card(
                    "Bootstrap Mode",
                    model.shell.bootstrap_mode_label.clone(),
                ))
                .child(self.settings_readonly_value_card("Config Path", config_path.clone())),
        )
        .child(
            div().h_flex().justify_end().child(
                self.settings_action_button(
                    "settings-copy-config-path",
                    "Copy Path",
                    theme::accent_cyan(),
                    cx,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.copy_settings_config_path(config_path.clone(), cx);
                })),
            ),
        )
    }
}
