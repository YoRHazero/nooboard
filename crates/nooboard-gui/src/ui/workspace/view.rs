use gpui::{
    AppContext as _, Context, Div, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui::prelude::FluentBuilder as _;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{Disableable, StyledExt, TitleBar};

use crate::{
    ui::theme,
    workspace::{
        LaunchHandle,
        actions::{self as workspace_actions, WorkspaceRoute},
        controller::{RecentActivitySeverity, WorkspaceController},
        view_state::{WorkspaceMetricViewState, WorkspaceViewState, build_workspace_view_state},
    },
};

pub struct WorkspaceView {
    controller: Entity<WorkspaceController>,
}

impl WorkspaceView {
    pub fn new(launch: LaunchHandle, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let controller = cx.new(|_| WorkspaceController::new(launch.clone()));
        cx.observe(&controller, |_, _, cx| cx.notify()).detach();
        WorkspaceController::initialize(&controller, launch, cx);

        Self { controller }
    }

    fn view_state(&self, cx: &Context<Self>) -> WorkspaceViewState {
        let controller = self.controller.read(cx);
        let recent_activity = controller
            .recent_activity()
            .iter()
            .cloned()
            .collect::<Vec<_>>();

        build_workspace_view_state(
            controller.route(),
            controller.load_state(),
            controller.snapshot(),
            controller.latest_committed_record(),
            &recent_activity,
            controller.bridge_state(),
            controller.launch().config_path.display().to_string(),
        )
    }

    fn current_route(&self, cx: &Context<Self>) -> WorkspaceRoute {
        self.controller.read(cx).route()
    }

    fn nav_item(
        &self,
        id: &'static str,
        label: &'static str,
        route: WorkspaceRoute,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active = self.current_route(cx) == route;

        div()
            .id(id)
            .cursor_pointer()
            .px(px(14.0))
            .py(px(12.0))
            .rounded(px(18.0))
            .bg(if active {
                theme::bg_panel_highlight()
            } else {
                theme::bg_console()
            })
            .border_1()
            .border_color(if active {
                theme::border_strong()
            } else {
                theme::border_soft()
            })
            .hover(|this| this.bg(theme::bg_panel_alt()))
            .on_click(cx.listener(move |this, _, _, cx| {
                let _ = this.controller.update(cx, |controller, cx| {
                    controller.set_route(route);
                    cx.notify();
                });
            }))
            .child(
                div()
                    .text_size(px(13.0))
                    .font_semibold()
                    .text_color(if active {
                        theme::fg_primary()
                    } else {
                        theme::fg_secondary()
                    })
                    .child(label),
            )
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> Div {
        div()
            .w(px(216.0))
            .min_h_0()
            .bg(theme::bg_sidebar())
            .border_1()
            .border_color(theme::border_base())
            .rounded(px(26.0))
            .child(
                div()
                    .v_flex()
                    .h_full()
                    .gap(px(10.0))
                    .p(px(18.0))
                    .child(self.nav_item("nav-home", "Home", WorkspaceRoute::Home, cx))
                    .child(self.nav_item(
                        "nav-clipboard",
                        "Clipboard",
                        WorkspaceRoute::Clipboard,
                        cx,
                    ))
                    .child(self.nav_item("nav-network", "Network", WorkspaceRoute::Network, cx))
                    .child(self.nav_item(
                        "nav-transfers",
                        "Transfers",
                        WorkspaceRoute::Transfers,
                        cx,
                    ))
                    .child(self.nav_item(
                        "nav-settings",
                        "Settings",
                        WorkspaceRoute::Settings,
                        cx,
                    )),
            )
    }

    fn titlebar_chip(&self, label: &str, value: String, accent: gpui::Hsla) -> Div {
        div()
            .h_flex()
            .h(px(22.0))
            .gap(px(8.0))
            .items_center()
            .px(px(8.0))
            .bg(theme::bg_panel_alt())
            .rounded(px(999.0))
            .child(div().size(px(6.0)).rounded(px(999.0)).bg(accent))
            .child(
                div()
                    .text_size(px(10.0))
                    .font_semibold()
                    .text_color(theme::fg_secondary())
                    .child(label.to_uppercase()),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .font_semibold()
                    .text_color(theme::fg_primary())
                    .child(value),
            )
    }

    fn metric_card(&self, metric: &WorkspaceMetricViewState) -> Div {
        div()
            .v_flex()
            .gap(px(10.0))
            .p(px(16.0))
            .bg(theme::bg_panel())
            .border_1()
            .border_color(theme::border_soft())
            .rounded(px(20.0))
            .min_w(px(150.0))
            .child(
                div()
                    .text_size(px(11.0))
                    .font_semibold()
                    .text_color(theme::fg_muted())
                    .child(metric.label),
            )
            .child(
                div()
                    .text_size(px(18.0))
                    .font_semibold()
                    .text_color(theme::fg_primary())
                    .child(metric.value.clone()),
            )
    }

    fn stream_badge(&self, label: &str, open: bool) -> Div {
        let accent = if open {
            theme::accent_green()
        } else {
            theme::accent_rose()
        };

        div()
            .h_flex()
            .items_center()
            .gap(px(8.0))
            .px(px(10.0))
            .py(px(6.0))
            .rounded(px(999.0))
            .bg(theme::bg_panel())
            .border_1()
            .border_color(accent.opacity(0.24))
            .child(div().size(px(7.0)).rounded(px(999.0)).bg(accent))
            .child(
                div()
                    .text_size(px(11.0))
                    .font_semibold()
                    .text_color(theme::fg_secondary())
                    .child(format!("{label}: {}", if open { "open" } else { "closed" })),
            )
    }

    fn toolbar_button(
        &self,
        id: &'static str,
        label: &str,
        enabled: bool,
        accent: gpui::Hsla,
        on_click: impl Fn(&mut Self, &gpui::ClickEvent, &mut Window, &mut Context<Self>) + 'static,
        cx: &Context<Self>,
    ) -> Button {
        Button::new(id)
            .primary()
            .disabled(!enabled)
            .border_1()
            .border_color(accent.opacity(0.24))
            .on_click(cx.listener(on_click))
            .child(
                div()
                    .px(px(14.0))
                    .py(px(8.0))
                    .text_size(px(12.0))
                    .font_semibold()
                    .child(label.to_string()),
            )
    }

    fn list_card(&self, title: &str, items: &[String], empty: &str) -> Div {
        div()
            .v_flex()
            .gap(px(10.0))
            .p(px(18.0))
            .bg(theme::bg_panel())
            .border_1()
            .border_color(theme::border_soft())
            .rounded(px(22.0))
            .child(
                div()
                    .text_size(px(12.0))
                    .font_semibold()
                    .text_color(theme::fg_secondary())
                    .child(title.to_string()),
            )
            .children(if items.is_empty() {
                vec![
                    div()
                        .text_size(px(12.0))
                        .text_color(theme::fg_muted())
                        .child(empty.to_string())
                        .into_any_element(),
                ]
            } else {
                items.iter()
                    .map(|item| {
                        div()
                            .text_size(px(12.0))
                            .line_height(px(18.0))
                            .text_color(theme::fg_primary())
                            .child(item.clone())
                            .into_any_element()
                    })
                    .collect()
            })
    }

    fn activity_row(
        &self,
        item: &crate::workspace::view_state::RecentActivityViewState,
        row_index: usize,
    ) -> impl IntoElement {
        let accent = match item.severity {
            RecentActivitySeverity::Info => theme::accent_cyan(),
            RecentActivitySeverity::Warning => theme::accent_amber(),
            RecentActivitySeverity::Error => theme::accent_rose(),
        };

        div()
            .id(format!("activity-row-{row_index}"))
            .w_full()
            .h_flex()
            .items_start()
            .gap(px(12.0))
            .p(px(12.0))
            .bg(theme::bg_panel())
            .border_1()
            .border_color(theme::border_soft())
            .rounded(px(18.0))
            .child(div().mt(px(4.0)).size(px(8.0)).rounded(px(999.0)).bg(accent))
            .child(
                div()
                    .v_flex()
                    .flex_1()
                    .gap(px(4.0))
                    .child(
                        div()
                            .h_flex()
                            .justify_between()
                            .gap(px(10.0))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_semibold()
                                    .text_color(theme::fg_secondary())
                                    .child(item.label),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(theme::fg_muted())
                                    .child(item.time_label.clone()),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .line_height(px(18.0))
                            .text_color(theme::fg_primary())
                            .child(item.title.clone()),
                    ),
            )
    }

    fn header_panel(&self, state: &WorkspaceViewState) -> Div {
        div()
            .v_flex()
            .gap(px(6.0))
            .child(
                div()
                    .text_size(px(30.0))
                    .font_semibold()
                    .text_color(theme::fg_primary())
                    .child(state.headline.clone()),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(theme::fg_secondary())
                    .child(state.subheadline.clone()),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(theme::fg_muted())
                    .child(state.revision_label.clone()),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(theme::fg_muted())
                    .child(state.config_path_label.clone()),
            )
    }

    fn status_panel(&self, state: &WorkspaceViewState) -> Div {
        div()
            .h_flex()
            .flex_wrap()
            .gap(px(10.0))
            .child(self.stream_badge("State", state.state_stream_open))
            .child(self.stream_badge("Event", state.event_stream_open))
            .when_some(state.bridge_error.clone(), |this, message| {
                this.child(
                    div()
                        .px(px(10.0))
                        .py(px(6.0))
                        .rounded(px(999.0))
                        .bg(theme::accent_rose().opacity(0.12))
                        .border_1()
                        .border_color(theme::accent_rose().opacity(0.24))
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(theme::fg_primary())
                                .child(message),
                        ),
                )
            })
    }

    fn home_page(&self, state: &WorkspaceViewState) -> Vec<gpui::AnyElement> {
        vec![
            div()
                .h_flex()
                .flex_wrap()
                .gap(px(12.0))
                .children(state.metrics.iter().map(|metric| self.metric_card(metric)))
                .into_any_element(),
            self.list_card(
                "Latest Clipboard",
                &[
                    state.latest_clipboard_preview.clone(),
                    state
                        .latest_clipboard_event_id
                        .clone()
                        .map(|value| format!("Event: {value}"))
                        .unwrap_or_else(|| "Event: n/a".to_string()),
                    state
                        .latest_clipboard_source
                        .clone()
                        .map(|value| format!("Source: {value}"))
                        .unwrap_or_else(|| "Source: n/a".to_string()),
                ],
                "No clipboard data.",
            )
            .into_any_element(),
            div()
                .v_flex()
                .gap(px(10.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_semibold()
                        .text_color(theme::fg_secondary())
                        .child("RECENT ACTIVITY"),
                )
                .children(
                    state
                        .recent_activity
                        .iter()
                        .enumerate()
                        .map(|(index, item)| self.activity_row(item, index)),
                )
                .into_any_element(),
        ]
    }

    fn clipboard_page(&self, state: &WorkspaceViewState, cx: &Context<Self>) -> Vec<gpui::AnyElement> {
        vec![
            div()
                .h_flex()
                .gap(px(10.0))
                .child(self.toolbar_button(
                    "clipboard-adopt-latest",
                    "Adopt Latest",
                    state.can_adopt_latest,
                    theme::accent_green(),
                    |this, _, _, cx| workspace_actions::adopt_latest_clipboard(&this.controller, cx),
                    cx,
                ))
                .into_any_element(),
            self.list_card(
                "Clipboard Detail",
                &[
                    state
                        .latest_clipboard_event_id
                        .clone()
                        .map(|value| format!("Event: {value}"))
                        .unwrap_or_else(|| "Event: n/a".to_string()),
                    state
                        .latest_clipboard_source
                        .clone()
                        .map(|value| format!("Source: {value}"))
                        .unwrap_or_else(|| "Source: n/a".to_string()),
                    state.latest_clipboard_preview.clone(),
                ],
                "No clipboard record available.",
            )
            .into_any_element(),
        ]
    }

    fn network_page(&self, state: &WorkspaceViewState, cx: &Context<Self>) -> Vec<gpui::AnyElement> {
        vec![
            div()
                .h_flex()
                .gap(px(10.0))
                .child(self.toolbar_button(
                    "network-start",
                    "Start Network",
                    state.network_can_start,
                    theme::accent_green(),
                    |this, _, _, cx| workspace_actions::start_network(&this.controller, cx),
                    cx,
                ))
                .child(self.toolbar_button(
                    "network-stop",
                    "Stop Network",
                    state.network_can_stop,
                    theme::accent_rose(),
                    |this, _, _, cx| workspace_actions::stop_network(&this.controller, cx),
                    cx,
                ))
                .into_any_element(),
            self.list_card(
                "Network Status",
                &[state.network_status.clone()],
                "Network status unavailable.",
            )
            .into_any_element(),
            self.list_card("LAN Peers", &state.lan_peers, "No LAN peers discovered.")
                .into_any_element(),
            self.list_card("Direct Seeds", &state.direct_seeds, "No direct seeds configured.")
                .into_any_element(),
            self.list_card(
                "Pending Direct Requests",
                &state.pending_requests,
                "No pending direct requests.",
            )
            .into_any_element(),
            self.list_card("Sessions", &state.sessions, "No active sessions.")
                .into_any_element(),
        ]
    }

    fn transfers_page(&self, state: &WorkspaceViewState) -> Vec<gpui::AnyElement> {
        vec![
            self.list_card(
                "Incoming Transfers",
                &state.incoming_transfers,
                "No incoming transfers.",
            )
            .into_any_element(),
            self.list_card("Active Transfers", &state.active_transfers, "No active transfers.")
                .into_any_element(),
            self.list_card(
                "Completed Transfers",
                &state.completed_transfers,
                "No completed transfers.",
            )
            .into_any_element(),
        ]
    }

    fn settings_page(&self, state: &WorkspaceViewState) -> Vec<gpui::AnyElement> {
        vec![
            div()
                .h_flex()
                .flex_wrap()
                .gap(px(12.0))
                .children(state.settings_rows.iter().map(|metric| self.metric_card(metric)))
                .into_any_element(),
        ]
    }

    fn page_body(&self, state: &WorkspaceViewState, cx: &Context<Self>) -> Vec<gpui::AnyElement> {
        match state.route {
            WorkspaceRoute::Home => self.home_page(state),
            WorkspaceRoute::Clipboard => self.clipboard_page(state, cx),
            WorkspaceRoute::Network => self.network_page(state, cx),
            WorkspaceRoute::Transfers => self.transfers_page(state),
            WorkspaceRoute::Settings => self.settings_page(state),
        }
    }

    fn main_panel(&self, cx: &Context<Self>) -> Div {
        let state = self.view_state(cx);

        div()
            .flex_1()
            .min_h_0()
            .bg(theme::bg_canvas())
            .border_1()
            .border_color(theme::border_base())
            .rounded(px(28.0))
            .child(
                div()
                    .v_flex()
                    .size_full()
                    .gap(px(18.0))
                    .p(px(24.0))
                    .child(self.header_panel(&state))
                    .child(self.status_panel(&state))
                    .children(self.page_body(&state, cx)),
            )
    }
}

impl Render for WorkspaceView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.view_state(cx);
        let session_count = state
            .metrics
            .iter()
            .find(|metric| metric.label == "Sessions")
            .map(|metric| metric.value.clone())
            .unwrap_or_else(|| "0".to_string());
        let transfer_count = state
            .metrics
            .iter()
            .find(|metric| metric.label == "Transfers")
            .map(|metric| metric.value.clone())
            .unwrap_or_else(|| "0".to_string());

        div()
            .v_flex()
            .size_full()
            .bg(theme::bg_app())
            .text_color(theme::fg_primary())
            .child(
                TitleBar::new().child(
                    div()
                        .h_flex()
                        .h_full()
                        .w_full()
                        .justify_between()
                        .items_center()
                        .px(px(14.0))
                        .bg(theme::bg_sidebar())
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_semibold()
                                .child("Nooboard Control"),
                        )
                        .child(
                            div()
                                .h_flex()
                                .gap(px(8.0))
                                .items_center()
                                .child(
                                    self.titlebar_chip("Sessions", session_count, theme::accent_cyan()),
                                )
                                .child(self.titlebar_chip(
                                    "Transfers",
                                    transfer_count,
                                    theme::accent_amber(),
                                )),
                        ),
                ),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .gap(px(18.0))
                    .p(px(18.0))
                    .child(self.sidebar(cx))
                    .child(self.main_panel(cx)),
            )
    }
}
