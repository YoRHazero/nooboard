mod clipboard;
mod components;
mod home;
mod peers;
mod settings;
mod shared;
mod sidebar;
mod transfer_rail;
mod transfers;

use std::sync::Arc;

use gpui::{
    Context, Div, Entity, InteractiveElement, IntoElement, ParentElement, Render, ScrollHandle,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_component::scroll::ScrollableElement;
use gpui_component::{StyledExt, TitleBar};

use crate::state::{
    SharedState, WorkspaceRoute,
    live_app::{DesktopLiveApp, LiveAppStore},
};
use crate::ui::theme;

use self::clipboard::ClipboardPageState;
use self::components::{titlebar_brand, titlebar_chip};
use self::peers::PeersPageState;
use self::settings::{SettingsPageState, build_settings_snapshot};
use self::shared::MAIN_CANVAS_MIN_WIDTH;
use self::transfers::TransfersPageState;

pub struct WorkspaceView {
    state: Arc<SharedState>,
    live_store: Entity<LiveAppStore>,
    route: WorkspaceRoute,
    main_y_scroll: ScrollHandle,
    clipboard_page: ClipboardPageState,
    peers_page_state: PeersPageState,
    settings_page_state: SettingsPageState,
    transfers_page_state: TransfersPageState,
    transfer_rail_expanded: bool,
    transfer_rail_has_toggled: bool,
}

impl WorkspaceView {
    pub fn new(window: &mut Window, state: Arc<SharedState>, cx: &mut Context<Self>) -> Self {
        let live_store = cx.global::<DesktopLiveApp>().store();
        cx.observe(&live_store, |this, _, cx| {
            this.sync_settings_page_state(cx);
            cx.notify();
        })
        .detach();

        let clipboard_page = ClipboardPageState::new(&state.app.clipboard);
        let peers_page_state = PeersPageState::new();
        let settings_snapshot = {
            let store = live_store.read(cx);
            build_settings_snapshot(&store)
        };
        let settings_page_state = SettingsPageState::new(settings_snapshot, window, cx);
        let transfers_page_state = TransfersPageState::new();

        Self {
            state,
            live_store,
            route: WorkspaceRoute::Home,
            main_y_scroll: ScrollHandle::default(),
            clipboard_page,
            peers_page_state,
            settings_page_state,
            transfers_page_state,
            transfer_rail_expanded: true,
            transfer_rail_has_toggled: false,
        }
    }

    fn main_viewport(&self, main: Div) -> Div {
        div()
            .flex_1()
            .min_w(px(0.0))
            .min_h_0()
            .bg(theme::bg_canvas())
            .border_1()
            .border_color(theme::border_base())
            .rounded(px(28.0))
            .overflow_hidden()
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
                                div().w_full().p(px(22.0)).child(
                                    div()
                                        .w_full()
                                        .overflow_x_scrollbar()
                                        .child(self.main_canvas(main)),
                                ),
                            ),
                    )
                    .vertical_scrollbar(&self.main_y_scroll),
            )
    }

    fn main_canvas(&self, main: Div) -> Div {
        div().w_full().min_w(px(MAIN_CANVAS_MIN_WIDTH)).child(main)
    }

    fn workspace_shell(&self, main: Div, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_row()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .gap(px(18.0))
            .p(px(18.0))
            .child(self.sidebar(cx).h_full())
            .child(self.main_viewport(main).h_full())
            .child(self.transfer_rail(cx))
    }
}

impl Render for WorkspaceView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app_state = self.live_store.read(cx).app_state().clone();
        let main = match self.route {
            WorkspaceRoute::Home => self.home_page(cx),
            WorkspaceRoute::Clipboard => self.clipboard_page(cx),
            WorkspaceRoute::Peers => self.peers_page(cx),
            WorkspaceRoute::Transfers => self.transfers_page(cx),
            WorkspaceRoute::Settings => self.settings_page(cx),
        };

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
                        .child(titlebar_brand())
                        .child(
                            div()
                                .h_flex()
                                .gap(px(8.0))
                                .items_center()
                                .child(titlebar_chip(
                                    "Peers",
                                    app_state.peers.connected.len().to_string(),
                                    theme::accent_cyan(),
                                ))
                                .child(titlebar_chip(
                                    "Inbox",
                                    app_state.transfers.incoming_pending.len().to_string(),
                                    theme::accent_amber(),
                                )),
                        ),
                ),
            )
            .child(self.workspace_shell(main, cx))
    }
}
