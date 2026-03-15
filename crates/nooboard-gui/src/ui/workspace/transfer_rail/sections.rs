use gpui::{
    AnyElement, ClickEvent, Context, Hsla, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_component::StyledExt;

use crate::{
    ui::theme,
    workspace::view_state::{
        ActiveTransferViewState, CompletedTransferViewState, IncomingTransferViewState,
        TransfersPageViewState,
    },
};

use super::super::WorkspaceView;

impl WorkspaceView {
    pub(in crate::ui::workspace::transfer_rail) fn transfer_sections(
        &self,
        state: &TransfersPageViewState,
        on_open: impl Fn(&mut Self, &ClickEvent, &mut Window, &mut Context<Self>) + Clone + 'static,
        cx: &Context<Self>,
    ) -> gpui::Div {
        let incoming_cards = state
            .incoming
            .iter()
            .enumerate()
            .take(3)
            .map(|(index, item)| {
                self.rail_incoming_transfer_card(index, item, on_open.clone(), cx)
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        let active_cards = state
            .active
            .iter()
            .enumerate()
            .take(3)
            .map(|(index, item)| {
                self.rail_active_transfer_card(index, item, on_open.clone(), cx)
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        let completed_cards = state
            .completed
            .iter()
            .enumerate()
            .take(3)
            .map(|(index, item)| {
                self.rail_completed_transfer_card(index, item, on_open.clone(), cx)
                    .into_any_element()
            })
            .collect::<Vec<_>>();

        div()
            .v_flex()
            .gap(px(18.0))
            .child(self.rail_transfer_section(
                "Awaiting",
                state.incoming.len(),
                incoming_cards,
                "No files awaiting review.",
            ))
            .child(self.rail_transfer_section(
                "Active",
                state.active.len(),
                active_cards,
                "No files in progress.",
            ))
            .child(self.rail_transfer_section(
                "Completed",
                state.completed.len(),
                completed_cards,
                "No completed files.",
            ))
    }

    fn rail_transfer_section(
        &self,
        title: &str,
        count: usize,
        cards: Vec<AnyElement>,
        empty_label: &str,
    ) -> gpui::Div {
        let content = if cards.is_empty() {
            vec![
                div()
                    .p(px(14.0))
                    .bg(theme::bg_activity())
                    .border_1()
                    .border_color(theme::border_soft())
                    .rounded(px(18.0))
                    .text_size(px(12.0))
                    .text_color(theme::fg_muted())
                    .child(empty_label.to_string())
                    .into_any_element(),
            ]
        } else {
            cards
        };

        div()
            .v_flex()
            .gap(px(12.0))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_semibold()
                            .text_color(theme::fg_primary())
                            .child(title.to_string()),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme::fg_muted())
                            .child(count.to_string()),
                    ),
            )
            .children(content)
    }

    fn rail_incoming_transfer_card(
        &self,
        index: usize,
        item: &IncomingTransferViewState,
        on_open: impl Fn(&mut Self, &ClickEvent, &mut Window, &mut Context<Self>) + 'static,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(("transfer-rail-incoming", index))
            .cursor_pointer()
            .on_click(cx.listener(on_open))
            .v_flex()
            .gap(px(10.0))
            .p(px(14.0))
            .bg(theme::bg_rail_panel())
            .border_1()
            .border_color(theme::border_soft())
            .rounded(px(18.0))
            .hover(|this| this.bg(theme::bg_panel_highlight()))
            .child(self.rail_transfer_card_heading(&item.file_name, theme::accent_amber()))
            .child(self.rail_transfer_card_meta(&item.peer_device_id, &item.size_label))
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::fg_muted())
                    .line_clamp(1)
                    .text_ellipsis()
                    .child(item.peer_noob_id.clone()),
            )
    }

    fn rail_active_transfer_card(
        &self,
        index: usize,
        item: &ActiveTransferViewState,
        on_open: impl Fn(&mut Self, &ClickEvent, &mut Window, &mut Context<Self>) + 'static,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(("transfer-rail-active", index))
            .cursor_pointer()
            .on_click(cx.listener(on_open))
            .v_flex()
            .gap(px(10.0))
            .p(px(14.0))
            .bg(theme::bg_rail_panel())
            .border_1()
            .border_color(theme::border_soft())
            .rounded(px(18.0))
            .hover(|this| this.bg(theme::bg_panel_highlight()))
            .child(self.rail_transfer_card_heading(&item.file_name, theme::accent_blue()))
            .child(self.rail_transfer_card_meta(&item.peer_device_id, &item.direction_label))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(10.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme::fg_secondary())
                            .child(item.progress_label.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_semibold()
                            .text_color(theme::accent_blue())
                            .child(item.state_label.clone()),
                    ),
            )
    }

    fn rail_completed_transfer_card(
        &self,
        index: usize,
        item: &CompletedTransferViewState,
        on_open: impl Fn(&mut Self, &ClickEvent, &mut Window, &mut Context<Self>) + 'static,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let secondary = item
            .saved_path
            .as_ref()
            .map(|path| path.display().to_string())
            .or_else(|| item.message.clone())
            .unwrap_or_else(|| item.peer_noob_id.clone());

        div()
            .id(("transfer-rail-complete", index))
            .cursor_pointer()
            .on_click(cx.listener(on_open))
            .v_flex()
            .gap(px(10.0))
            .p(px(14.0))
            .bg(theme::bg_rail_panel())
            .border_1()
            .border_color(theme::border_soft())
            .rounded(px(18.0))
            .hover(|this| this.bg(theme::bg_panel_highlight()))
            .child(self.rail_transfer_card_heading(&item.file_name, theme::accent_green()))
            .child(self.rail_transfer_card_meta(&item.peer_device_id, &item.outcome_label))
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::fg_muted())
                    .line_clamp(2)
                    .text_ellipsis()
                    .child(secondary),
            )
    }

    fn rail_transfer_card_heading(&self, file_name: &str, accent: Hsla) -> gpui::Div {
        div()
            .h_flex()
            .items_center()
            .gap(px(10.0))
            .child(div().size(px(8.0)).rounded(px(999.0)).bg(accent))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .text_size(px(12.0))
                    .font_semibold()
                    .text_color(theme::fg_primary())
                    .line_clamp(1)
                    .text_ellipsis()
                    .child(file_name.to_string()),
            )
    }

    fn rail_transfer_card_meta(&self, primary: &str, secondary: &str) -> gpui::Div {
        div()
            .h_flex()
            .items_center()
            .justify_between()
            .gap(px(10.0))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .text_size(px(11.0))
                    .text_color(theme::fg_secondary())
                    .line_clamp(1)
                    .text_ellipsis()
                    .child(primary.to_string()),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::fg_muted())
                    .child(secondary.to_string()),
            )
    }
}
