use gpui::prelude::FluentBuilder as _;
use gpui::{Context, IntoElement, ParentElement, Styled, div, px};
use gpui_component::{Disableable, StyledExt};

use crate::{
    ui::theme,
    workspace::view_state::{NetworkDirectSeedViewState, NetworkPageViewState},
};

use super::super::WorkspaceView;

impl WorkspaceView {
    pub(in crate::ui::workspace::network) fn network_seed_panel(
        &self,
        state: &NetworkPageViewState,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        self.network_panel_shell(
            "Direct Seeds",
            format!("{} configured", state.direct_seed_count),
        )
        .children(if state.direct_seeds.is_empty() {
            vec![
                self.network_empty_notice("No direct seeds configured.")
                    .into_any_element(),
            ]
        } else {
            state
                .direct_seeds
                .iter()
                .cloned()
                .map(|seed| self.network_seed_row(seed, cx).into_any_element())
                .collect()
        })
    }

    pub(in crate::ui::workspace::network) fn network_seed_row(
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
                                        .map(|value| format!("learned device {value}"))
                                        .unwrap_or_else(|| "no learned device id yet".to_string()),
                                ),
                        )
                        .when_some(seed.last_connected_addr_label.clone(), |this, value| {
                            this.child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(theme::fg_muted())
                                    .child(format!("last connected {value}")),
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
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    this.request_network_edit_seed(&seed_for_edit, window, cx);
                                },
                            )),
                        )
                        .child(
                            self.network_action_button(
                                format!("network-seed-connect-{}", seed.id),
                                "Connect",
                                theme::accent_green(),
                                cx,
                            )
                            .disabled(pending)
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.request_network_connect_seed(&seed_for_connect, cx);
                                },
                            )),
                        )
                        .child(
                            self.network_action_button(
                                format!("network-seed-remove-{}", seed.id),
                                "Remove",
                                theme::accent_rose(),
                                cx,
                            )
                            .disabled(pending)
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.request_network_remove_seed(&seed_for_remove, cx);
                                },
                            )),
                        ),
                ),
        )
    }
}
