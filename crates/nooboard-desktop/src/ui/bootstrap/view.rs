use std::path::{Path, PathBuf};

use gpui::{
    Context, IntoElement, ParentElement, PathPromptOptions, Render, SharedString, Styled, Window,
    div, prelude::FluentBuilder, px,
};
use gpui_component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_component::{Disableable, Icon, IconName, StyledExt, TITLE_BAR_HEIGHT};
use nooboard_config::{
    AppConfig, BootstrapChooserContext, BootstrapLaunch, BootstrapMode, ConfigTemplate,
    DEFAULT_CONFIG_FILE_NAME, repo_development_config_path, write_config_template,
};
use tokio::sync::oneshot;

use super::state::{BootstrapChooserState, BootstrapPreset, CustomLocationSelection};
use crate::ui::theme;

const ACTION_BUTTON_WIDTH: f32 = 116.0;
const ACTION_BUTTON_HEIGHT: f32 = 40.0;

pub struct BootstrapChooserView {
    chooser_context: BootstrapChooserContext,
    chooser_state: BootstrapChooserState,
    can_use_repo_development: bool,
    launch_sender: Option<oneshot::Sender<BootstrapLaunch>>,
    launch_in_flight: bool,
    feedback: Option<String>,
}

impl BootstrapChooserView {
    pub fn new(
        chooser_context: BootstrapChooserContext,
        can_use_repo_development: bool,
        launch_sender: oneshot::Sender<BootstrapLaunch>,
    ) -> Self {
        Self {
            chooser_context,
            chooser_state: BootstrapChooserState::default(),
            can_use_repo_development,
            launch_sender: Some(launch_sender),
            launch_in_flight: false,
            feedback: None,
        }
    }

    fn begin_action(&mut self, cx: &mut Context<Self>) -> bool {
        if self.launch_in_flight || self.launch_sender.is_none() {
            return false;
        }

        self.launch_in_flight = true;
        self.feedback = None;
        cx.notify();
        true
    }

    fn end_action(&mut self, cx: &mut Context<Self>) {
        self.launch_in_flight = false;
        cx.notify();
    }

    fn finish_launch(
        &mut self,
        launch: BootstrapLaunch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(sender) = self.launch_sender.take() {
            let _ = sender.send(launch);
        }
        self.launch_in_flight = false;
        cx.notify();
        window.remove_window();
    }

    fn fail_action(&mut self, message: String, cx: &mut Context<Self>) {
        self.launch_in_flight = false;
        self.feedback = Some(message);
        cx.notify();
    }

    fn set_selected_preset(&mut self, preset: BootstrapPreset, cx: &mut Context<Self>) {
        self.feedback = None;
        self.chooser_state.select_preset(preset);
        cx.notify();
    }

    fn quit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.launch_sender.take();
        self.launch_in_flight = false;
        cx.notify();
        window.remove_window();
    }

    fn start_default_configuration(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.begin_action(cx) {
            return;
        }

        let config_path = self.chooser_context.default_config_path.clone();
        match write_config_template(&config_path, ConfigTemplate::Production) {
            Ok(()) => self.finish_launch(
                BootstrapLaunch {
                    mode: BootstrapMode::UserDefault,
                    config_path,
                },
                window,
                cx,
            ),
            Err(error) => self.fail_action(error.to_string(), cx),
        }
    }

    fn browse_existing_config(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.begin_action(cx) {
            return;
        }

        let paths_receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose existing nooboard config".into()),
        });

        cx.spawn_in(window, async move |view, cx| {
            let path = match paths_receiver.await {
                Ok(Ok(Some(mut paths))) => paths.drain(..).next(),
                _ => None,
            };

            let _ = view.update(cx, |this, cx| {
                match path {
                    Some(path) => this.apply_existing_config_selection(path),
                    None => this.end_action(cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn browse_custom_location(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.begin_action(cx) {
            return;
        }

        let paths_receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose directory for nooboard config".into()),
        });

        cx.spawn_in(window, async move |view, cx| {
            let directory = match paths_receiver.await {
                Ok(Ok(Some(mut paths))) => paths.drain(..).next(),
                _ => None,
            };

            let _ = view.update(cx, |this, cx| {
                match directory {
                    Some(directory) => this.apply_custom_location_selection(directory),
                    None => this.end_action(cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn apply_existing_config_selection(&mut self, path: PathBuf) {
        self.launch_in_flight = false;
        self.feedback = None;
        match AppConfig::load(&path) {
            Ok(_) => self.chooser_state.set_existing_config_valid(path),
            Err(error) => self
                .chooser_state
                .set_existing_config_invalid(path, error.to_string()),
        }
    }

    fn apply_custom_location_selection(&mut self, directory: PathBuf) {
        self.launch_in_flight = false;
        self.feedback = None;

        let config_path = directory.join(DEFAULT_CONFIG_FILE_NAME);
        if !config_path.exists() {
            self.chooser_state
                .set_custom_location_ready_to_create(directory);
            return;
        }

        match AppConfig::load(&config_path) {
            Ok(_) => self
                .chooser_state
                .set_custom_location_existing_config(directory),
            Err(error) => self
                .chooser_state
                .set_custom_location_invalid_config(directory, error.to_string()),
        }
    }

    fn rewrite_existing_config(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self
            .chooser_state
            .existing_config
            .path()
            .map(Path::to_path_buf)
        else {
            return;
        };

        if !self.begin_action(cx) {
            return;
        }

        match write_config_template(&path, ConfigTemplate::Production) {
            Ok(()) => {
                self.chooser_state.set_existing_config_valid(path);
                self.feedback = None;
                self.end_action(cx);
            }
            Err(error) => self.fail_action(error.to_string(), cx),
        }
    }

    fn confirm_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.chooser_state.selected_preset {
            BootstrapPreset::DefaultConfig => self.start_default_configuration(window, cx),
            BootstrapPreset::ExistingConfig => self.confirm_existing_config(window, cx),
            BootstrapPreset::CustomLocation => self.confirm_custom_location(window, cx),
            BootstrapPreset::RepoDevelopment => self.use_repo_development_config(window, cx),
        }
    }

    fn confirm_existing_config(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(config_path) = self
            .chooser_state
            .existing_config
            .valid_path()
            .map(Path::to_path_buf)
        else {
            return;
        };

        if !self.begin_action(cx) {
            return;
        }

        match AppConfig::load(&config_path) {
            Ok(_) => self.finish_launch(
                BootstrapLaunch {
                    mode: BootstrapMode::ExplicitPath,
                    config_path,
                },
                window,
                cx,
            ),
            Err(error) => {
                self.chooser_state
                    .set_existing_config_invalid(config_path, error.to_string());
                self.fail_action(error.to_string(), cx);
            }
        }
    }

    fn confirm_custom_location(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let selection = self.chooser_state.custom_location.clone();
        if !self.begin_action(cx) {
            return;
        }

        match selection {
            CustomLocationSelection::ReadyToCreate {
                directory,
                config_path,
            } => {
                let result = if config_path.exists() {
                    AppConfig::load(&config_path).map(|_| ())
                } else {
                    write_config_template(&config_path, ConfigTemplate::Production)
                };

                match result {
                    Ok(()) => {
                        self.chooser_state
                            .set_custom_location_existing_config(directory);
                        self.finish_launch(
                            BootstrapLaunch {
                                mode: BootstrapMode::ExplicitPath,
                                config_path,
                            },
                            window,
                            cx,
                        );
                    }
                    Err(error) => {
                        self.chooser_state
                            .set_custom_location_invalid_config(directory, error.to_string());
                        self.fail_action(error.to_string(), cx);
                    }
                }
            }
            CustomLocationSelection::ExistingConfig {
                directory,
                config_path,
            } => match AppConfig::load(&config_path) {
                Ok(_) => self.finish_launch(
                    BootstrapLaunch {
                        mode: BootstrapMode::ExplicitPath,
                        config_path,
                    },
                    window,
                    cx,
                ),
                Err(error) => {
                    self.chooser_state
                        .set_custom_location_invalid_config(directory, error.to_string());
                    self.fail_action(error.to_string(), cx);
                }
            },
            CustomLocationSelection::None | CustomLocationSelection::InvalidConfig { .. } => {
                self.end_action(cx);
            }
        }
    }

    fn use_repo_development_config(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.begin_action(cx) {
            return;
        }

        match repo_development_config_path() {
            Ok(config_path) if config_path.exists() => self.finish_launch(
                BootstrapLaunch {
                    mode: BootstrapMode::RepoDevelopment,
                    config_path,
                },
                window,
                cx,
            ),
            Ok(config_path) => self.fail_action(
                format!(
                    "repository development config not found at {}",
                    config_path.display()
                ),
                cx,
            ),
            Err(error) => self.fail_action(error.to_string(), cx),
        }
    }

    fn preset_button(
        &self,
        id: &'static str,
        preset: BootstrapPreset,
        cx: &Context<Self>,
    ) -> Button {
        let selected = self.chooser_state.selected_preset == preset;
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
            .disabled(self.launch_in_flight)
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

    fn action_button(
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

    fn action_placeholder(&self) -> impl IntoElement {
        div().w(px(ACTION_BUTTON_WIDTH)).h(px(ACTION_BUTTON_HEIGHT))
    }
}

impl Render for BootstrapChooserView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let description = self.chooser_state.description(&self.chooser_context);
        let confirm_enabled = self
            .chooser_state
            .confirm_enabled(self.can_use_repo_development)
            && !self.launch_in_flight;
        let browse_enabled = self.chooser_state.browse_enabled() && !self.launch_in_flight;
        let rewrite_visible = self.chooser_state.rewrite_visible();
        let rewrite_enabled = rewrite_visible && !self.launch_in_flight;

        let mut preset_row = div()
            .w_full()
            .v_flex()
            .gap(px(12.0))
            .child(
                self.preset_button("bootstrap-default", BootstrapPreset::DefaultConfig, cx)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.set_selected_preset(BootstrapPreset::DefaultConfig, cx);
                    })),
            )
            .child(
                self.preset_button("bootstrap-existing", BootstrapPreset::ExistingConfig, cx)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.set_selected_preset(BootstrapPreset::ExistingConfig, cx);
                    })),
            )
            .child(
                self.preset_button("bootstrap-custom", BootstrapPreset::CustomLocation, cx)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.set_selected_preset(BootstrapPreset::CustomLocation, cx);
                    })),
            );

        if self.can_use_repo_development {
            preset_row = preset_row.child(
                self.preset_button("bootstrap-dev", BootstrapPreset::RepoDevelopment, cx)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.set_selected_preset(BootstrapPreset::RepoDevelopment, cx);
                    })),
            );
        }

        let browse_button = if self.chooser_state.browse_enabled() {
            self.action_button(
                "bootstrap-browse",
                "Browse",
                theme::accent_cyan(),
                !browse_enabled,
                cx,
            )
            .on_click(cx.listener(
                |this, _, window, cx| match this.chooser_state.selected_preset {
                    BootstrapPreset::ExistingConfig => this.browse_existing_config(window, cx),
                    BootstrapPreset::CustomLocation => this.browse_custom_location(window, cx),
                    BootstrapPreset::DefaultConfig | BootstrapPreset::RepoDevelopment => {}
                },
            ))
            .into_any_element()
        } else {
            self.action_placeholder().into_any_element()
        };

        let rewrite_button = if rewrite_visible {
            self.action_button(
                "bootstrap-rewrite",
                "Rewrite",
                theme::accent_amber(),
                !rewrite_enabled,
                cx,
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.rewrite_existing_config(cx);
            }))
            .into_any_element()
        } else {
            self.action_placeholder().into_any_element()
        };

        div()
            .size_full()
            .bg(theme::bg_app())
            .text_color(theme::fg_primary())
            .child({
                let top_inset = TITLE_BAR_HEIGHT + px(16.0);
                let bottom_inset = px(20.0);
                div()
                    .size_full()
                    .px(px(20.0))
                    .pt(top_inset)
                    .pb(bottom_inset)
                    .flex()
                    .items_start()
                    .justify_center()
                    .child(
                        div()
                            .w(px(560.0))
                            .max_w_full()
                            .v_flex()
                            .gap(px(20.0))
                            .bg(theme::bg_panel())
                            .border_1()
                            .border_color(theme::border_base())
                            .rounded(px(28.0))
                            .shadow_xs()
                            .p(px(22.0))
                            .child(
                                div()
                                    .h_flex()
                                    .items_center()
                                    .gap(px(10.0))
                                    .child(
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
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(22.0))
                                            .font_semibold()
                                            .text_color(theme::fg_primary())
                                            .child("Choose Configuration"),
                                    ),
                            )
                            .child(preset_row)
                            .child(
                                div()
                                    .w_full()
                                    .min_h(px(118.0))
                                    .rounded(px(20.0))
                                    .border_1()
                                    .border_color(theme::border_soft())
                                    .bg(theme::bg_console())
                                    .p(px(18.0))
                                    .v_flex()
                                    .gap(px(10.0))
                                    .child(
                                        div()
                                            .text_size(px(14.0))
                                            .font_semibold()
                                            .text_color(theme::fg_primary())
                                            .child(self.chooser_state.selected_preset.title()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .line_height(px(20.0))
                                            .whitespace_normal()
                                            .text_color(theme::fg_secondary())
                                            .child(description),
                                    ),
                            )
                            .when_some(self.feedback.clone(), |this, message| {
                                this.child(
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
                                        .child(SharedString::from(message)),
                                )
                            })
                            .child(
                                div()
                                    .w_full()
                                    .h_flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .h_flex()
                                            .gap(px(10.0))
                                            .child(browse_button)
                                            .child(rewrite_button),
                                    )
                                    .child(
                                        div()
                                            .h_flex()
                                            .gap(px(10.0))
                                            .child(
                                                self.action_button(
                                                    "bootstrap-confirm",
                                                    "Confirm",
                                                    theme::accent_green(),
                                                    !confirm_enabled,
                                                    cx,
                                                )
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.confirm_selection(window, cx);
                                                })),
                                            )
                                            .child(
                                                self.action_button(
                                                    "bootstrap-quit",
                                                    "Quit",
                                                    theme::accent_rose(),
                                                    self.launch_in_flight,
                                                    cx,
                                                )
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.quit(window, cx);
                                                })),
                                            ),
                                    ),
                            ),
                    )
            })
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
