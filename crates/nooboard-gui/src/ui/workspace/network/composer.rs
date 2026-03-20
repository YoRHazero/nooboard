use gpui::{Context, IntoElement, ParentElement, Styled, div, px};
use gpui_component::input::Input;
use gpui_component::{Disableable, Sizable, StyledExt};

use crate::ui::theme;

use super::{super::WorkspaceView, SeedPanelMode};

impl WorkspaceView {
    pub(in crate::ui::workspace::network) fn network_seed_controls(
        &self,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        div()
            .v_flex()
            .gap(px(12.0))
            .child(
                div()
                    .h_flex()
                    .flex_wrap()
                    .gap(px(8.0))
                    .child(self.network_segment_button(
                        "network-seed-mode-create",
                        "Create",
                        self.network.seed_panel_mode() == SeedPanelMode::Create,
                        cx.listener(|this, _, _, cx| {
                            this.request_network_set_seed_panel_mode(SeedPanelMode::Create, cx);
                        }),
                    ))
                    .child(self.network_segment_button(
                        "network-seed-mode-search",
                        "Search",
                        self.network.seed_panel_mode() == SeedPanelMode::Search,
                        cx.listener(|this, _, _, cx| {
                            this.request_network_set_seed_panel_mode(SeedPanelMode::Search, cx);
                        }),
                    )),
            )
            .child(match self.network.seed_panel_mode() {
                SeedPanelMode::Create => self.network_seed_editor(cx).into_any_element(),
                SeedPanelMode::Search => self.network_seed_search_bar(cx).into_any_element(),
            })
    }

    pub(in crate::ui::workspace::network) fn network_seed_editor(
        &self,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let editing = self.network.editing_seed_id().is_some();

        self.network_row_shell()
            .child(
                div()
                    .h_flex()
                    .flex_wrap()
                    .gap(px(12.0))
                    .child(
                        self.network_input_field(
                            "Label",
                            Input::new(&self.network.seed_label_input())
                                .small()
                                .appearance(false)
                                .bordered(false)
                                .focus_bordered(false)
                                .w_full(),
                        ),
                    )
                    .child(
                        self.network_input_field(
                            "Host",
                            Input::new(&self.network.seed_host_input())
                                .small()
                                .appearance(false)
                                .bordered(false)
                                .focus_bordered(false)
                                .w_full(),
                        ),
                    )
                    .child(
                        self.network_input_field(
                            "Port",
                            Input::new(&self.network.seed_port_input())
                                .small()
                                .appearance(false)
                                .bordered(false)
                                .focus_bordered(false)
                                .w_full(),
                        ),
                    )
                    .child(self.network_toggle_chip(
                        self.network.draft_enabled(),
                        "Enabled",
                        theme::accent_blue(),
                        cx,
                    )),
            )
            .child(
                div()
                    .h_flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme::fg_muted())
                            .child(if editing {
                                "Editing an existing direct preset."
                            } else {
                                "Create a reusable direct preset for manual connections."
                            }),
                    )
                    .child(
                        div()
                            .h_flex()
                            .flex_wrap()
                            .gap(px(8.0))
                            .child(
                                self.network_action_button(
                                    "network-seed-save",
                                    if editing { "Save" } else { "Create" },
                                    theme::accent_blue(),
                                    cx,
                                )
                                .disabled(self.network.save_in_flight())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.request_network_save_seed(cx);
                                })),
                            )
                            .child(
                                self.network_action_button(
                                    "network-seed-clear",
                                    "Reset",
                                    theme::accent_rose(),
                                    cx,
                                )
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.request_network_clear_seed(window, cx);
                                })),
                            ),
                    ),
            )
    }

    pub(in crate::ui::workspace::network) fn network_seed_search_bar(
        &self,
        _cx: &Context<Self>,
    ) -> impl IntoElement {
        self.network_row_shell()
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::fg_muted())
                    .child("Filter saved presets by label or learned device id."),
            )
            .child(
                self.network_input_field(
                    "Filter",
                    Input::new(&self.network.seed_filter_input())
                        .small()
                        .appearance(false)
                        .bordered(false)
                        .focus_bordered(false)
                        .w_full(),
                ),
            )
    }
}
