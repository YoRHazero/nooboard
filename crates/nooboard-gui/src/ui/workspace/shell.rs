use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, Div, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_component::scroll::ScrollableElement;
use gpui_component::{StyledExt, TitleBar};

use crate::{ui::theme, workspace::route::WorkspaceRoute};

use super::{
    WorkspaceRenderModel, WorkspaceView,
    shared::{MAIN_CANVAS_MIN_WIDTH, SIDEBAR_WIDTH},
};

impl WorkspaceView {
    fn sidebar(&self, cx: &mut Context<Self>) -> Div {
        div()
            .w(px(SIDEBAR_WIDTH))
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
                    .child(self.nav_item("nav-settings", "Settings", WorkspaceRoute::Settings, cx)),
            )
    }

    fn header_panel(&self, model: &WorkspaceRenderModel) -> Div {
        div()
            .v_flex()
            .gap(px(6.0))
            .child(
                div()
                    .text_size(px(30.0))
                    .font_semibold()
                    .text_color(theme::fg_primary())
                    .child(model.shell.headline.clone()),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(theme::fg_secondary())
                    .child(model.shell.subheadline.clone()),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(theme::fg_muted())
                    .child(model.shell.config_path_label.clone()),
            )
    }

    fn status_panel(&self, model: &WorkspaceRenderModel) -> Div {
        div()
            .h_flex()
            .flex_wrap()
            .gap(px(10.0))
            .child(self.stream_badge("State", model.shell.state_stream_open))
            .child(self.stream_badge("Event", model.shell.event_stream_open))
            .when_some(model.shell.bridge_error.clone(), |this, message| {
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

    fn page_body(
        &self,
        model: &WorkspaceRenderModel,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        match model.route {
            WorkspaceRoute::Home => self.home_page(model, cx),
            WorkspaceRoute::Clipboard => self.clipboard_page(model, window, cx),
            WorkspaceRoute::Network => self.network_page(model, cx),
            WorkspaceRoute::Transfers => self.transfers_page(model, cx),
            WorkspaceRoute::Settings => self.settings_page(model, cx),
        }
    }

    fn main_canvas(
        &self,
        model: &WorkspaceRenderModel,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .w_full()
            .min_w(px(MAIN_CANVAS_MIN_WIDTH))
            .child(self.main_panel(model, window, cx))
    }

    fn main_panel(
        &self,
        model: &WorkspaceRenderModel,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .w_full()
            .v_flex()
            .gap(px(18.0))
            .child(self.header_panel(model))
            .child(self.status_panel(model))
            .children(self.page_body(model, window, cx))
    }

    fn main_viewport(
        &self,
        model: &WorkspaceRenderModel,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .flex_1()
            .min_w(px(0.0))
            .min_h_0()
            .bg(theme::bg_canvas())
            .border_1()
            .border_color(theme::border_base())
            .rounded(px(28.0))
            .child(
                div()
                    .relative()
                    .size_full()
                    .min_h_0()
                    .child(
                        div()
                            .id("workspace-main-y-scroll")
                            .size_full()
                            .track_scroll(&self.main_y_scroll)
                            .overflow_y_scroll()
                            .child(
                                div().w_full().p(px(24.0)).child(
                                    div()
                                        .w_full()
                                        .overflow_x_scrollbar()
                                        .child(self.main_canvas(model, window, cx)),
                                ),
                            ),
                    )
                    .vertical_scrollbar(&self.main_y_scroll),
            )
    }

    fn workspace_shell(
        &self,
        model: &WorkspaceRenderModel,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .flex()
            .flex_row()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .gap(px(18.0))
            .p(px(18.0))
            .child(self.sidebar(cx).h_full())
            .child(self.main_viewport(model, window, cx).h_full())
            .child(self.transfer_rail(model, cx))
    }

    pub(super) fn render_root(
        &self,
        model: WorkspaceRenderModel,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let session_count = model
            .page
            .as_ref()
            .map(|state| state.shell_metrics.session_count_label.clone())
            .unwrap_or_else(|| "0".to_string());
        let transfer_count = model
            .page
            .as_ref()
            .map(|state| state.shell_metrics.inbox_count_label.clone())
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
                        .child(self.titlebar_brand())
                        .child(
                            div()
                                .h_flex()
                                .gap(px(8.0))
                                .items_center()
                                .child(self.titlebar_chip(
                                    "Sessions",
                                    session_count,
                                    theme::accent_cyan(),
                                ))
                                .child(self.titlebar_chip(
                                    "Inbox",
                                    transfer_count,
                                    theme::accent_amber(),
                                )),
                        ),
                ),
            )
            .child(self.workspace_shell(&model, window, cx))
    }
}
