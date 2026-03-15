use gpui::{
    Context, IntoElement, ParentElement, PathPromptOptions, Render, SharedString, Styled, Window,
    div, px,
};
use gpui::prelude::FluentBuilder as _;
use gpui_component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_component::{Disableable, Icon, IconName, StyledExt, TITLE_BAR_HEIGHT};
use nooboard_core::{BootstrapChooserContext, BootstrapLaunch};
use tokio::sync::oneshot;

use crate::{
    bootstrap::{BootstrapController, BootstrapPreset},
    ui::theme,
};

const ACTION_BUTTON_WIDTH: f32 = 116.0;
const ACTION_BUTTON_HEIGHT: f32 = 40.0;

pub struct BootstrapChooserView {
    controller: BootstrapController,
}

impl BootstrapChooserView {
    pub fn new(
        context: BootstrapChooserContext,
        can_use_repo_development: bool,
        launch_sender: oneshot::Sender<BootstrapLaunch>,
    ) -> Self {
        Self {
            controller: BootstrapController::new(
                context,
                can_use_repo_development,
                launch_sender,
            ),
        }
    }

    fn browse_existing_config(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.controller.begin_action() {
            return;
        }
        cx.notify();

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
                    Some(path) => this.controller.apply_existing_config_selection(path),
                    None => this.controller.end_action(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn browse_custom_location(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.controller.begin_action() {
            return;
        }
        cx.notify();

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
                    Some(directory) => this.controller.apply_custom_location_selection(directory),
                    None => this.controller.end_action(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn finish_launch(
        &mut self,
        launch: BootstrapLaunch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.controller.send_launch(launch);
        cx.notify();
        window.remove_window();
    }

    fn confirm_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(launch) = self.controller.confirm_selection() else {
            cx.notify();
            return;
        };

        self.finish_launch(launch, window, cx);
    }

    fn quit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.controller.quit();
        cx.notify();
        window.remove_window();
    }

    fn preset_button(
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
        let view_state = self.controller.view_state();

        let mut preset_row = div()
            .w_full()
            .v_flex()
            .gap(px(12.0))
            .child(
                self.preset_button("bootstrap-default", BootstrapPreset::DefaultConfig, cx)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.controller
                            .set_selected_preset(BootstrapPreset::DefaultConfig);
                        cx.notify();
                    })),
            )
            .child(
                self.preset_button("bootstrap-existing", BootstrapPreset::ExistingConfig, cx)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.controller
                            .set_selected_preset(BootstrapPreset::ExistingConfig);
                        cx.notify();
                    })),
            )
            .child(
                self.preset_button("bootstrap-custom", BootstrapPreset::CustomLocation, cx)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.controller
                            .set_selected_preset(BootstrapPreset::CustomLocation);
                        cx.notify();
                    })),
            );

        if self.controller.can_use_repo_development() {
            preset_row = preset_row.child(
                self.preset_button("bootstrap-dev", BootstrapPreset::RepoDevelopment, cx)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.controller
                            .set_selected_preset(BootstrapPreset::RepoDevelopment);
                        cx.notify();
                    })),
            );
        }

        let browse_button = if view_state.browse_enabled {
            self.action_button(
                "bootstrap-browse",
                "Browse",
                theme::accent_cyan(),
                !view_state.browse_enabled,
                cx,
            )
            .on_click(cx.listener(
                |this, _, window, cx| match this.controller.selected_preset() {
                    BootstrapPreset::ExistingConfig => this.browse_existing_config(window, cx),
                    BootstrapPreset::CustomLocation => this.browse_custom_location(window, cx),
                    BootstrapPreset::DefaultConfig | BootstrapPreset::RepoDevelopment => {}
                },
            ))
            .into_any_element()
        } else {
            self.action_placeholder().into_any_element()
        };

        let rewrite_button = if view_state.rewrite_visible {
            self.action_button(
                "bootstrap-rewrite",
                "Rewrite",
                theme::accent_amber(),
                !view_state.rewrite_enabled,
                cx,
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.controller.rewrite_existing_config();
                cx.notify();
            }))
            .into_any_element()
        } else {
            self.action_placeholder().into_any_element()
        };

        div().size_full().bg(theme::bg_app()).text_color(theme::fg_primary()).child({
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
                                        .child(view_state.chooser_title),
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
                                        .child(view_state.selected_preset.title()),
                                )
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .line_height(px(20.0))
                                        .whitespace_normal()
                                        .text_color(theme::fg_secondary())
                                        .child(view_state.description),
                                ),
                        )
                        .child(
                            div().w_full().h(px(24.0)).child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(theme::fg_muted())
                                    .child(format!(
                                        "Repo development preset: {}",
                                        if view_state.can_use_repo_development {
                                            "available"
                                        } else {
                                            "unavailable"
                                        }
                                    )),
                            ),
                        )
                        .when_some(view_state.feedback.clone(), |this, message| {
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
                                                !view_state.confirm_enabled,
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
                                                view_state.launch_in_flight,
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
