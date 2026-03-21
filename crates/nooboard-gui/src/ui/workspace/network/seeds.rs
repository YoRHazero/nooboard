use gpui::prelude::FluentBuilder as _;
use gpui::{Context, IntoElement, ParentElement, Styled, div, px};
use gpui_component::{Disableable, StyledExt, scroll::ScrollableElement};

use crate::{
    ui::theme,
    workspace::view_state::{NetworkDirectSeedViewState, NetworkPageViewState},
};

use super::super::WorkspaceView;

const DIRECT_LIST_HEIGHT: f32 = 420.0;

impl WorkspaceView {
    pub(in crate::ui::workspace::network) fn network_seed_tab(
        &self,
        state: &NetworkPageViewState,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let seeds = self.network.filtered_direct_seeds(state, cx);
        let filter_empty = self.network.seed_filter(cx).trim().is_empty();

        div()
            .v_flex()
            .gap(px(14.0))
            .child(self.network_seed_controls(cx))
            .child(
                div()
                    .v_flex()
                    .gap(px(10.0))
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .justify_between()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_semibold()
                                    .text_color(theme::fg_secondary())
                                    .child("SAVED DEVICES"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(theme::fg_muted())
                                    .child(format!("{} visible", seeds.len())),
                            ),
                    )
                    .child(
                        div()
                            .h(px(DIRECT_LIST_HEIGHT))
                            .overflow_y_scrollbar()
                            .child(
                                div()
                                    .v_flex()
                                    .gap(px(10.0))
                                    .children(if seeds.is_empty() {
                                        vec![
                                            self.network_empty_notice(if filter_empty {
                                                "No saved devices yet."
                                            } else {
                                                "No saved devices match your search."
                                            })
                                            .into_any_element(),
                                        ]
                                    } else {
                                        seeds
                                            .into_iter()
                                            .map(|seed| {
                                                self.network_seed_card(seed, cx).into_any_element()
                                            })
                                            .collect()
                                    }),
                            ),
                    ),
            )
    }

    pub(in crate::ui::workspace::network) fn network_seed_card(
        &self,
        seed: NetworkDirectSeedViewState,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let pending = self.network.seed_pending(seed.id);
        let seed_for_edit = seed.clone();
        let seed_for_connect = seed.clone();
        let seed_for_remove = seed.clone();

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
                                .h_flex()
                                .items_center()
                                .gap(px(8.0))
                                .child(
                                    div()
                                        .text_size(px(14.0))
                                        .font_semibold()
                                        .text_color(theme::fg_primary())
                                        .child(seed.label.clone()),
                                )
                                .child(self.network_status_chip(
                                    if seed.enabled { "Enabled" } else { "Disabled" },
                                    &seed.endpoint_label,
                                    if seed.enabled {
                                        theme::accent_green()
                                    } else {
                                        theme::accent_amber()
                                    },
                                )),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(theme::fg_muted())
                                        .child(
                                    seed.learned_device_id
                                        .clone()
                                        .map(|value| format!("Learned device code {value}"))
                                        .unwrap_or_else(|| "No learned device code yet".to_string()),
                                ),
                        )
                        .when_some(seed.last_connected_addr_label.clone(), |this, value| {
                            this.child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(theme::fg_muted())
                                    .child(format!("Last connected {value}")),
                            )
                        }),
                )
                .child(
                    div()
                        .h_flex()
                        .flex_wrap()
                        .justify_end()
                        .gap(px(8.0))
                        .child(
                            self.network_action_button(
                                format!("network-seed-edit-{}", seed.id),
                                "Edit",
                                theme::accent_cyan(),
                                cx,
                            )
                            .disabled(pending)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.request_network_edit_seed(&seed_for_edit, window, cx);
                            })),
                        )
                        .child(
                            self.network_action_button(
                                format!("network-seed-connect-{}", seed.id),
                                "Connect",
                                theme::accent_green(),
                                cx,
                            )
                            .disabled(pending || !seed.enabled)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.request_network_connect_seed(&seed_for_connect, cx);
                            })),
                        )
                        .child(
                            self.network_action_button(
                                format!("network-seed-remove-{}", seed.id),
                                "Delete",
                                theme::accent_rose(),
                                cx,
                            )
                            .disabled(pending)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.request_network_remove_seed(&seed_for_remove, cx);
                            })),
                        ),
                ),
        )
    }
}
