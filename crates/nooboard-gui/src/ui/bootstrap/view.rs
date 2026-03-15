use gpui::{Context, IntoElement, ParentElement, Render, Styled, div, px};
use gpui_component::{StyledExt, TITLE_BAR_HEIGHT};

use crate::{
    bootstrap::BootstrapPreset,
    ui::{BootstrapChooserView, theme},
};

impl Render for BootstrapChooserView {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
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

        let mut panel = div()
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
                    .child(self.chooser_title_icon())
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
            );

        if let Some(message) = view_state.feedback.clone() {
            panel = panel.child(self.feedback_banner(message));
        }

        panel = panel.child(
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
                            .on_click(cx.listener(
                                |this, _, window, cx| {
                                    this.confirm_selection(window, cx);
                                },
                            )),
                        )
                        .child(
                            self.action_button(
                                "bootstrap-quit",
                                "Quit",
                                theme::accent_rose(),
                                view_state.launch_in_flight,
                                cx,
                            )
                            .on_click(cx.listener(
                                |this, _, window, cx| {
                                    this.quit(window, cx);
                                },
                            )),
                        ),
                ),
        );

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
                    .child(panel)
            })
    }
}
