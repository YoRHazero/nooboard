use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    div, px,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::Input;
use gpui_component::{Disableable, IconName, Sizable, StyledExt};

use crate::{ui::theme, workspace::view_state::SettingsPageViewState};

use super::super::super::WorkspaceView;
use super::super::state::SettingsSectionKey;

impl WorkspaceView {
    pub(in crate::ui::workspace::settings) fn transfer_settings_panel(
        &self,
        _state: &SettingsPageViewState,
        dirty: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let (status_label, status_accent) =
            self.settings_section_status(SettingsSectionKey::Transfers, dirty);
        let actions_enabled = dirty && self.settings.applying().is_none();

        self.settings_section_shell(
            "Transfers",
            "Choose where files from other devices are saved.",
            self.settings_status_chip(status_label, status_accent),
        )
        .child(
            div()
                .min_w(px(240.0))
                .flex_1()
                .v_flex()
                .gap(px(6.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme::fg_secondary())
                        .child("Save Incoming Files To"),
                )
                .child(
                    div()
                        .px(px(12.0))
                        .py(px(10.0))
                        .bg(theme::bg_console())
                        .border_1()
                        .border_color(theme::border_soft())
                        .rounded(px(16.0))
                        .child(
                            Input::new(&self.settings.download_dir_input())
                                .small()
                                .appearance(false)
                                .bordered(false)
                                .focus_bordered(false)
                                .suffix(
                                    div()
                                        .id("settings-browse-download-dir-shell")
                                        .tooltip(move |window, cx| {
                                            Self::settings_themed_tooltip(
                                                "Choose download directory".to_string(),
                                                window,
                                                cx,
                                            )
                                        })
                                        .child(
                                            Button::new("settings-browse-download-dir")
                                                .ghost()
                                                .xsmall()
                                                .icon(IconName::FolderOpen)
                                                .disabled(
                                                    self.settings.applying().is_some(),
                                                )
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.pick_settings_download_dir(window, cx);
                                                })),
                                        ),
                                )
                                .w_full(),
                        ),
                ),
        )
        .child(
            div()
                .h_flex()
                .justify_end()
                .gap(px(8.0))
                .child(self.settings_compact_action_button(
                    "settings-reset-transfers",
                    "Reset",
                    "Undo the changes in this section."
                        .to_string(),
                    actions_enabled,
                    theme::accent_rose(),
                    |this, _, window, cx| {
                        this.request_reset_transfer_settings(window, cx);
                    },
                    cx,
                ))
                .child(self.settings_compact_action_button(
                        "settings-apply-transfers",
                        "Apply",
                        "Save the changes in this section.".to_string(),
                        actions_enabled,
                        theme::accent_cyan(),
                        |this, _, _, cx| {
                        this.request_apply_transfer_settings(cx);
                    },
                        cx,
                )),
        )
    }
}
