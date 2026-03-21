use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    Window, div, px,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{IconName, Sizable, StyledExt};

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
                .child(self.settings_readonly_value_card_with_action(
                    "Config Path",
                    config_path.clone(),
                    Some(
                        div()
                            .id("settings-copy-config-path-shell")
                            .tooltip(move |window: &mut Window, cx| {
                                Self::settings_themed_tooltip(
                                    "Copy config path".to_string(),
                                    window,
                                    cx,
                                )
                            })
                            .child(
                                Button::new("settings-copy-config-path")
                                    .ghost()
                                    .xsmall()
                                    .rounded(px(10.0))
                                    .icon(IconName::Copy)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.copy_settings_config_path(config_path.clone(), cx);
                                    })),
                            )
                            .into_any_element(),
                    ),
                )),
        )
    }
}
