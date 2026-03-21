use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{IconName, Sizable, StyledExt};

use crate::{
    ui::theme,
    workspace::view_state::NetworkPageViewState,
};

use super::{
    super::{WorkspaceRenderModel, WorkspaceView},
    DirectPanelTab,
};

impl WorkspaceView {
    pub(in crate::ui::workspace) fn network_page(
        &self,
        model: &WorkspaceRenderModel,
        cx: &Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(state) = model.page.as_ref().map(|page| &page.network) else {
            return vec![
                self.list_card(
                    "Network",
                    &["Loading your network status.".to_string()],
                    "Loading your network status.",
                )
                .into_any_element(),
            ];
        };

        let mut children = vec![self.network_header_panel(state, cx).into_any_element()];
        if let Some(message) = self.network.feedback() {
            children.push(
                self.network_feedback_banner(network_feedback_accent(message), message.clone())
                    .into_any_element(),
            );
        } else if state.status_label.starts_with("Error:") {
            children.push(
                self.network_feedback_banner(theme::accent_rose(), state.status_label.clone())
                    .into_any_element(),
            );
        }
        children.push(self.network_workspace_panels(state, cx).into_any_element());
        children
    }

    pub(in crate::ui::workspace::network) fn network_header_panel(
        &self,
        state: &NetworkPageViewState,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let token_value = if self.network.token_revealed() {
            state.network_token.clone()
        } else {
            mask_token(&state.network_token)
        };

        self.network_panel_shell(
            "Network",
            "View this device's connection details and control nearby sync and direct connections.",
        )
        .child(
            div()
                .h_flex()
                .flex_wrap()
                .gap(px(10.0))
                .child(self.network_meta_pill(
                    "Device",
                    &state.local_device_id,
                    theme::accent_cyan(),
                ))
                .child(self.network_meta_pill(
                    "Device Code",
                    &state.local_noob_id,
                    theme::accent_blue(),
                ))
                .child(self.network_meta_pill(
                    "Endpoint",
                    &state.endpoint_label,
                    theme::accent_green(),
                ))
                .child(
                    div()
                        .min_w(px(200.0))
                        .flex_1()
                        .v_flex()
                        .gap(px(6.0))
                        .px(px(12.0))
                        .py(px(10.0))
                        .bg(theme::bg_console())
                        .border_1()
                        .border_color(theme::border_soft())
                        .rounded(px(16.0))
                        .child(
                            div()
                                .h_flex()
                                .items_center()
                                .justify_between()
                                .gap(px(8.0))
                                .child(
                                    div()
                                        .text_size(px(10.0))
                                        .font_semibold()
                                        .text_color(theme::accent_rose())
                                        .child("TOKEN"),
                                )
                                .child(
                                    div()
                                        .id("network-token-tooltip")
                                        .tooltip({
                                            let text = if self.network.token_revealed() {
                                                "Hide token".to_string()
                                            } else {
                                                "Show token".to_string()
                                            };
                                            move |window, cx| {
                                                Self::network_themed_tooltip(
                                                    text.clone(),
                                                    window,
                                                    cx,
                                                )
                                            }
                                        })
                                        .child(
                                            Button::new("network-token-reveal")
                                                .ghost()
                                                .xsmall()
                                                .icon(if self.network.token_revealed() {
                                                    IconName::EyeOff
                                                } else {
                                                    IconName::Eye
                                                })
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.request_network_toggle_token_revealed(cx);
                                                })),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(theme::fg_primary())
                                .line_clamp(2)
                                .text_ellipsis()
                                .child(token_value),
                        ),
                ),
        )
        .child(
            div()
                .h_flex()
                .flex_wrap()
                .gap(px(12.0))
                .items_center()
                .justify_between()
                .child(
                    div()
                        .h_flex()
                        .gap(px(10.0))
                        .flex_wrap()
                        .child(
                            self.network_action_button(
                                "network-open-settings",
                                "Edit Settings",
                                theme::accent_cyan(),
                                cx,
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.request_network_open_settings(cx);
                            })),
                        ),
                )
                .child(
                    self.network_toggle_switch(
                        "network-runtime-toggle",
                        "Network Sharing",
                        state.network_enabled,
                        theme::accent_green(),
                        "Turn nearby sync and direct connections on or off.",
                        cx.listener(|this, _, _, cx| {
                            this.request_network_toggle_runtime(cx);
                        }),
                    ),
                ),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(theme::fg_muted())
                .child(format!("Status: {}", state.status_label)),
        )
        .child(
            div()
                .h_flex()
                .flex_wrap()
                .gap(px(10.0))
                .items_center()
                .child(self.network_metric_chip(
                    "Saved",
                    state.direct_seed_count.to_string(),
                    theme::accent_blue(),
                ))
                .child(self.network_metric_chip(
                    "Requests",
                    state.pending_request_count.to_string(),
                    theme::accent_amber(),
                ))
                .child(self.network_metric_chip(
                    "Direct",
                    state.direct_session_count.to_string(),
                    theme::accent_green(),
                ))
                .child(self.network_metric_chip(
                    "Nearby",
                    state.connected_lan_peer_count.to_string(),
                    theme::accent_cyan(),
                )),
        )
    }

    pub(in crate::ui::workspace::network) fn network_workspace_panels(
        &self,
        state: &NetworkPageViewState,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        div()
            .v_flex()
            .gap(px(18.0))
            .child(self.network_direct_panel(state, cx))
            .child(self.network_lan_panel(state, cx))
    }

    pub(in crate::ui::workspace::network) fn network_direct_panel(
        &self,
        state: &NetworkPageViewState,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        self.network_panel_shell(
            "Direct Connect",
            "Manage saved devices, incoming requests, and active direct connections.",
        )
        .child(
            div()
                .h_flex()
                .flex_wrap()
                .gap(px(8.0))
                .child(self.network_segment_button(
                    "network-direct-tab-seeds",
                    "Saved",
                    self.network.direct_tab() == DirectPanelTab::Seeds,
                    cx.listener(|this, _, _, cx| {
                        this.request_network_set_direct_tab(DirectPanelTab::Seeds, cx);
                    }),
                ))
                .child(self.network_segment_button(
                    "network-direct-tab-pending",
                    "Requests",
                    self.network.direct_tab() == DirectPanelTab::Pending,
                    cx.listener(|this, _, _, cx| {
                        this.request_network_set_direct_tab(DirectPanelTab::Pending, cx);
                    }),
                ))
                .child(self.network_segment_button(
                    "network-direct-tab-sessions",
                    "Connected",
                    self.network.direct_tab() == DirectPanelTab::Sessions,
                    cx.listener(|this, _, _, cx| {
                        this.request_network_set_direct_tab(DirectPanelTab::Sessions, cx);
                    }),
                )),
        )
        .child(match self.network.direct_tab() {
            DirectPanelTab::Seeds => self.network_seed_tab(state, cx).into_any_element(),
            DirectPanelTab::Pending => self.network_request_tab(state, cx).into_any_element(),
            DirectPanelTab::Sessions => self.network_session_tab(state, cx).into_any_element(),
        })
    }
}

fn network_feedback_accent(message: &str) -> gpui::Hsla {
    let lower = message.to_lowercase();
    if lower.contains("fail") || lower.contains("error") {
        theme::accent_rose()
    } else if lower.contains("disconnect") || lower.contains("remove") {
        theme::accent_amber()
    } else {
        theme::accent_cyan()
    }
}

fn mask_token(token: &str) -> String {
    if token.is_empty() {
        return "unset".to_string();
    }

    "*".repeat(token.chars().count().max(8))
}
