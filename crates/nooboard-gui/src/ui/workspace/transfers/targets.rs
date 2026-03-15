use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    div, px,
};
use gpui_component::StyledExt;

use crate::{
    ui::theme,
    workspace::view_state::{TransfersPageViewState, WorkspaceSessionTargetViewState},
};

use super::super::WorkspaceView;

impl WorkspaceView {
    pub(in crate::ui::workspace::transfers) fn transfers_target_panel(
        &self,
        state: &TransfersPageViewState,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        self.transfers_panel_shell(
            "Transfer Targets",
            format!("{} connected session(s)", state.available_targets.len()),
        )
        .child(div().h_flex().flex_wrap().gap(px(10.0)).children(
            if state.available_targets.is_empty() {
                vec![
                    self.transfers_empty_notice("No connected sessions available for uploads.")
                        .into_any_element(),
                ]
            } else {
                state
                    .available_targets
                    .iter()
                    .cloned()
                    .map(|target| self.transfer_target_chip(target, cx).into_any_element())
                    .collect()
            },
        ))
    }

    pub(in crate::ui::workspace::transfers) fn transfer_target_chip(
        &self,
        target: WorkspaceSessionTargetViewState,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let selected = self.transfers.selected_session_ids().contains(&target.id);

        div()
            .id(format!("transfer-target-{}", target.id))
            .cursor_pointer()
            .min_w(px(172.0))
            .px(px(12.0))
            .py(px(10.0))
            .rounded(px(16.0))
            .bg(if selected {
                theme::bg_panel_highlight()
            } else {
                theme::bg_console()
            })
            .border_1()
            .border_color(if selected {
                theme::accent_cyan().opacity(0.34)
            } else {
                theme::border_soft()
            })
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.0))
                    .child(
                        div()
                            .v_flex()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_semibold()
                                    .text_color(theme::fg_primary())
                                    .child(target.device_id.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(theme::fg_muted())
                                    .line_clamp(1)
                                    .text_ellipsis()
                                    .child(format!(
                                        "{} · {}",
                                        target.mode_label, target.remote_addr_label
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(div().size(px(6.0)).rounded(px(999.0)).bg(if selected {
                                theme::accent_cyan()
                            } else {
                                theme::border_base()
                            }))
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .font_semibold()
                                    .text_color(if selected {
                                        theme::accent_cyan()
                                    } else {
                                        theme::fg_secondary()
                                    })
                                    .child(if selected { "Selected" } else { "Connected" }),
                            ),
                    ),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.request_transfer_toggle_target(target.id, cx);
            }))
    }
}
