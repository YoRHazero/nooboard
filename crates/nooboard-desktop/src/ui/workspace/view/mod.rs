mod clipboard;
mod components;
mod home;
mod shared;
mod sidebar;
mod transfer_rail;
mod transfers;

use std::sync::Arc;

use gpui::{
    Context, Div, InteractiveElement, IntoElement, ParentElement, Render, ScrollHandle,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::scroll::ScrollableElement;
use gpui_component::{StyledExt, TitleBar};

use crate::state::{SharedState, TransferItem, TransferStage, WorkspaceRoute};
use crate::ui::theme;

use self::clipboard::ClipboardPageState;
use self::components::{titlebar_brand, titlebar_chip};
use self::shared::MAIN_CANVAS_MIN_WIDTH;
use self::transfers::TransfersPageState;

pub struct WorkspaceView {
    state: Arc<SharedState>,
    route: WorkspaceRoute,
    main_y_scroll: ScrollHandle,
    clipboard_page: ClipboardPageState,
    transfers_page_state: TransfersPageState,
    transfer_items: Vec<TransferItem>,
    transfer_rail_expanded: bool,
    transfer_rail_has_toggled: bool,
    network_service_enabled: bool,
    auto_bridge_remote_text: bool,
}

impl WorkspaceView {
    pub fn new(state: Arc<SharedState>) -> Self {
        let network_service_enabled = state.app.system_core.network_enabled;
        let auto_bridge_remote_text = state.app.system_core.auto_bridge_remote_text;
        let clipboard_page = ClipboardPageState::new(&state.app.clipboard);
        let transfers_page_state = TransfersPageState::new(&state.app.clipboard);
        let transfer_items = state.app.transfer_items.clone();

        Self {
            state,
            route: WorkspaceRoute::Home,
            main_y_scroll: ScrollHandle::default(),
            clipboard_page,
            transfers_page_state,
            transfer_items,
            transfer_rail_expanded: true,
            transfer_rail_has_toggled: false,
            network_service_enabled,
            auto_bridge_remote_text,
        }
    }

    fn placeholder_page(&self, label: &str, description: &str, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .justify_center()
            .w_full()
            .min_h(px(620.0))
            .child(
                div()
                    .v_flex()
                    .gap(px(14.0))
                    .p(px(30.0))
                    .bg(theme::bg_panel())
                    .border_1()
                    .border_color(theme::border_base())
                    .rounded(px(24.0))
                    .shadow_xs()
                    .child(
                        div()
                            .text_size(px(24.0))
                            .font_semibold()
                            .text_color(theme::fg_primary())
                            .child(label.to_string()),
                    )
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(theme::fg_secondary())
                            .line_clamp(3)
                            .text_ellipsis()
                            .child(description.to_string()),
                    )
                    .child(
                        Button::new("workspace-home")
                            .primary()
                            .label("Back to Home")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.route = WorkspaceRoute::Home;
                                cx.notify();
                            })),
                    ),
            )
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

    fn transfer_count(&self, stage: TransferStage) -> usize {
        self.transfer_items
            .iter()
            .filter(|item| item.stage() == stage)
            .count()
    }

    fn awaiting_review_count(&self) -> usize {
        self.transfer_count(TransferStage::AwaitingReview)
    }

    fn progress_count(&self) -> usize {
        self.transfer_count(TransferStage::Progress)
    }

    fn complete_count(&self) -> usize {
        self.transfer_count(TransferStage::Complete)
    }

    fn dismiss_complete_item(&mut self, item_id: &str, cx: &mut Context<Self>) {
        self.transfer_items
            .retain(|item| !(item.id == item_id && item.is_complete()));
        cx.notify();
    }
}

impl Render for WorkspaceView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let main = match self.route {
            WorkspaceRoute::Home => self.home_page(cx),
            WorkspaceRoute::Clipboard => self.clipboard_page(cx),
            WorkspaceRoute::Peers => self.placeholder_page(
                "Peers & Network",
                "Connected peers, manual peers, and runtime toggles will land here after the home dashboard settles.",
                cx,
            ),
            WorkspaceRoute::Transfers => self.transfers_page(cx),
            WorkspaceRoute::Settings => self.placeholder_page(
                "Settings",
                "Storage, network, and desktop behavior tuning will arrive here once the control surface is finalized.",
                cx,
            ),
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
                                    self.state.app.online_peers.to_string(),
                                    theme::accent_cyan(),
                                ))
                                .child(titlebar_chip(
                                    "Inbox",
                                    self.awaiting_review_count().to_string(),
                                    theme::accent_amber(),
                                )),
                        ),
                ),
            )
            .child(self.workspace_shell(main, cx))
    }
}
