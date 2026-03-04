use std::sync::Arc;

use gpui::{Context, Div, IntoElement, ParentElement, Render, Styled, Window, div, px};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::scroll::ScrollableElement;
use gpui_component::{StyledExt, TitleBar};

use crate::state::{PendingFileDecision, QuickPanelTab, SharedState};
use crate::ui::theme;

pub struct QuickPanelView {
    state: Arc<SharedState>,
    active_tab: QuickPanelTab,
}

impl QuickPanelView {
    pub fn new(state: Arc<SharedState>) -> Self {
        Self {
            state,
            active_tab: QuickPanelTab::Send,
        }
    }

    fn tab_button(
        &self,
        id: &'static str,
        label: &'static str,
        tab: QuickPanelTab,
        cx: &mut Context<Self>,
    ) -> Button {
        let button = Button::new(id)
            .label(label)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.active_tab = tab;
                cx.notify();
            }));

        if self.active_tab == tab {
            button.primary()
        } else {
            button
        }
    }

    fn section_title(&self, text: &str) -> Div {
        div()
            .text_size(px(14.0))
            .font_semibold()
            .text_color(theme::fg_primary())
            .child(text.to_string())
    }

    fn send_tab(&self) -> Div {
        div()
            .v_flex()
            .gap(px(16.0))
            .w_full()
            .child(self.section_title("Quick Send"))
            .child(
                div()
                    .p(px(16.0))
                    .min_h(px(220.0))
                    .bg(theme::bg_panel_alt())
                    .border_1()
                    .border_color(theme::border_base())
                    .rounded(px(18.0))
                    .text_color(theme::fg_muted())
                    .child("Paste or type a message to broadcast"),
            )
            .child(
                div()
                    .v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme::fg_secondary())
                            .child("Targets"),
                    )
                    .child(
                        div()
                            .p(px(12.0))
                            .bg(theme::bg_panel_alt())
                            .rounded(px(14.0))
                            .text_color(theme::fg_primary())
                            .child("All online peers"),
                    ),
            )
            .child(Button::new("quick-send").primary().label("Send"))
            .child(
                div()
                    .p(px(12.0))
                    .bg(theme::bg_panel_alt())
                    .rounded(px(14.0))
                    .text_size(px(12.0))
                    .text_color(theme::fg_muted())
                    .child("Result feedback will be shown here after AppService wiring is added."),
            )
    }

    fn inbox_tab(&self) -> Div {
        div()
            .v_flex()
            .gap(px(12.0))
            .w_full()
            .child(self.section_title("Inbox"))
            .children(
                self.state
                    .app
                    .pending_files
                    .iter()
                    .enumerate()
                    .map(|(index, item)| Self::pending_file_card(index, item)),
            )
            .child(
                div()
                    .p(px(12.0))
                    .bg(theme::bg_panel_alt())
                    .rounded(px(14.0))
                    .text_size(px(12.0))
                    .text_color(theme::fg_muted())
                    .line_clamp(2)
                    .text_ellipsis()
                    .child("Connection error: 192.168.1.9 rejected handshake during capability negotiation"),
            )
    }

    fn pending_file_card(index: usize, item: &PendingFileDecision) -> Div {
        div()
            .v_flex()
            .gap(px(10.0))
            .p(px(14.0))
            .bg(theme::bg_panel_alt())
            .border_1()
            .border_color(theme::border_base())
            .rounded(px(18.0))
            .child(
                div()
                    .w_full()
                    .text_color(theme::fg_primary())
                    .font_semibold()
                    .truncate()
                    .child(item.file_name.clone()),
            )
            .child(
                div()
                    .w_full()
                    .text_size(px(12.0))
                    .text_color(theme::fg_muted())
                    .truncate()
                    .child(format!("{} · {}", item.peer_label, item.size_label)),
            )
            .child(
                div()
                    .h_flex()
                    .gap(px(10.0))
                    .child(Button::new(("accept", index)).primary().label("Accept"))
                    .child(Button::new(("reject", index)).label("Reject")),
            )
    }

    fn recent_tab(&self) -> Div {
        div()
            .v_flex()
            .gap(px(12.0))
            .w_full()
            .child(self.section_title("Recent"))
            .children(
                self.state
                    .app
                    .recent_history
                    .iter()
                    .take(5)
                    .enumerate()
                    .map(|(index, item)| {
                        div()
                            .h_flex()
                            .justify_between()
                            .items_center()
                            .gap(px(12.0))
                            .p(px(12.0))
                            .bg(theme::bg_panel_alt())
                            .rounded(px(14.0))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .text_color(theme::fg_primary())
                                    .truncate()
                                    .child(item.clone()),
                            )
                            .child(Button::new(("recent-copy", index)).label("Copy"))
                    }),
            )
            .child(Button::new("open-history").label("Open Full History"))
    }
}

impl Render for QuickPanelView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.active_tab {
            QuickPanelTab::Send => self.send_tab(),
            QuickPanelTab::Inbox => self.inbox_tab(),
            QuickPanelTab::Recent => self.recent_tab(),
        };

        div()
            .v_flex()
            .size_full()
            .bg(theme::bg_canvas())
            .text_color(theme::fg_primary())
            .child(
                TitleBar::new().child(
                    div()
                        .h_flex()
                        .w_full()
                        .justify_between()
                        .items_center()
                        .px(px(14.0))
                        .child(
                            div()
                                .h_flex()
                                .gap(px(12.0))
                                .items_center()
                                .child(
                                    div()
                                        .font_semibold()
                                        .text_color(theme::fg_primary())
                                        .child("Nooboard Quick"),
                                )
                                .child(div().text_color(theme::fg_secondary()).child(format!(
                                    "Inbox {}",
                                    self.state.app.pending_files.len()
                                ))),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(theme::fg_muted())
                                .child("Esc to close"),
                        ),
                ),
            )
            .child(
                div()
                    .v_flex()
                    .flex_1()
                    .min_h_0()
                    .gap(px(16.0))
                    .p(px(16.0))
                    .child(
                        div()
                            .h_flex()
                            .gap(px(10.0))
                            .child(self.tab_button("tab-send", "Send", QuickPanelTab::Send, cx))
                            .child(self.tab_button("tab-inbox", "Inbox", QuickPanelTab::Inbox, cx))
                            .child(self.tab_button(
                                "tab-recent",
                                "Recent",
                                QuickPanelTab::Recent,
                                cx,
                            )),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .v_flex()
                            .gap(px(16.0))
                            .p(px(18.0))
                            .bg(theme::bg_panel())
                            .border_1()
                            .border_color(theme::border_base())
                            .rounded(px(20.0))
                            .child(
                                div()
                                    .flex_1()
                                    .min_h_0()
                                    .overflow_y_scrollbar()
                                    .child(content),
                            ),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .justify_between()
                    .items_center()
                    .px(px(16.0))
                    .py(px(12.0))
                    .bg(theme::bg_panel())
                    .border_t_1()
                    .border_color(theme::border_base())
                    .child(
                        Button::new("open-workspace")
                            .primary()
                            .label("Open Workspace"),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme::fg_muted())
                            .child("Quick panel shell v0"),
                    ),
            )
    }
}
