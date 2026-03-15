use gpui::prelude::FluentBuilder as _;
use gpui::{Context, IntoElement, ParentElement, Styled, div, px};
use gpui_component::input::Input;
use gpui_component::{Disableable, Sizable, StyledExt};

use crate::ui::theme;

use super::{super::WorkspaceView, NetworkSearchResult};

impl WorkspaceView {
    pub(in crate::ui::workspace::network) fn network_seed_composer(
        &self,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let editing = self.network.editing_seed_id().is_some();
        let search_results = self
            .network
            .search_results()
            .iter()
            .cloned()
            .map(|result| self.network_search_result_row(result, cx))
            .collect::<Vec<_>>();

        self.network_panel_shell(
            "Direct Connect Composer",
            "Edit and save direct seed endpoints.",
        )
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
                .gap(px(8.0))
                .items_center()
                .child(
                    self.network_action_button(
                        "network-seed-save",
                        if editing { "Save Seed" } else { "Create Seed" },
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
                        "Clear",
                        theme::accent_rose(),
                        cx,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.request_network_clear_seed(window, cx);
                    })),
                )
                .child(
                    self.network_action_button(
                        "network-seed-search",
                        if self.network.search_in_flight() {
                            "Searching..."
                        } else {
                            "Search"
                        },
                        theme::accent_cyan(),
                        cx,
                    )
                    .disabled(self.network.search_in_flight())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.request_network_search(cx);
                    })),
                )
                .child(
                    div().flex_1().min_w(px(220.0)).child(
                        Input::new(&self.network.search_input())
                            .small()
                            .appearance(false)
                            .bordered(false)
                            .focus_bordered(false)
                            .w_full(),
                    ),
                ),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(theme::fg_muted())
                .line_clamp(2)
                .text_ellipsis()
                .child(self.network.feedback().cloned().unwrap_or_else(|| {
                    if editing {
                        "Editing an existing direct seed from the live snapshot.".to_string()
                    } else {
                        "Create a direct seed, then connect it from the list below.".to_string()
                    }
                })),
        )
        .when(!search_results.is_empty(), |panel| {
            panel.child(
                div()
                    .v_flex()
                    .gap(px(10.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_semibold()
                            .text_color(theme::fg_secondary())
                            .child("SEARCH RESULTS"),
                    )
                    .children(search_results),
            )
        })
    }

    pub(in crate::ui::workspace::network) fn network_search_result_row(
        &self,
        result: NetworkSearchResult,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let load_result = result.clone();

        self.network_row_shell().child(
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
                                .text_size(px(14.0))
                                .font_semibold()
                                .text_color(theme::fg_primary())
                                .child(result.label.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(theme::fg_secondary())
                                .child(result.endpoint_label.clone()),
                        )
                        .when_some(result.learned_device_id.clone(), |this, value| {
                            this.child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(theme::fg_muted())
                                    .child(format!("learned device {value}")),
                            )
                        }),
                )
                .child(
                    self.network_action_button(
                        format!("network-search-load-{}", result.id),
                        "Load",
                        theme::accent_blue(),
                        cx,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.request_network_load_search_result(&load_result, window, cx);
                    })),
                ),
        )
    }
}
