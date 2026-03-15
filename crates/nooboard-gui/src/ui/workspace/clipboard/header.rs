use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    div, px,
};
use gpui_component::StyledExt;

use crate::ui::theme;

use super::{
    components::{clipboard_metric_chip, clipboard_panel_shell, clipboard_tab_chip},
    state::ClipboardBroadcastScope,
    view_state::ClipboardPageViewState,
};
use crate::ui::workspace::WorkspaceView;

impl WorkspaceView {
    pub(super) fn clipboard_header(&self, snapshot: &ClipboardPageViewState) -> impl IntoElement {
        clipboard_panel_shell()
            .h_flex()
            .items_center()
            .justify_between()
            .gap(px(14.0))
            .p(px(18.0))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(22.0))
                            .font_semibold()
                            .text_color(theme::fg_primary())
                            .child("Clipboard"),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme::fg_muted())
                            .child("committed history"),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .flex_wrap()
                    .gap(px(8.0))
                    .child(clipboard_metric_chip(
                        "Sessions",
                        snapshot.connected_target_count.to_string(),
                        theme::accent_cyan(),
                    ))
                    .child(clipboard_metric_chip(
                        "History",
                        snapshot.loaded_history_count.to_string(),
                        theme::accent_amber(),
                    ))
                    .child(clipboard_metric_chip(
                        "Broadcast",
                        match snapshot.broadcast_scope {
                            ClipboardBroadcastScope::AllConnected => "All connected".to_string(),
                            ClipboardBroadcastScope::SelectedSessions => {
                                format!("{} selected", snapshot.selected_target_count)
                            }
                        },
                        theme::accent_green(),
                    )),
            )
    }

    pub(super) fn clipboard_targets_panel(
        &self,
        snapshot: &ClipboardPageViewState,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let scope_label = match snapshot.broadcast_scope {
            ClipboardBroadcastScope::AllConnected => "All sessions",
            ClipboardBroadcastScope::SelectedSessions => "Selected sessions",
        };

        clipboard_panel_shell()
            .v_flex()
            .gap(px(14.0))
            .p(px(18.0))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.0))
                    .child(
                        div()
                            .v_flex()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_semibold()
                                    .text_color(theme::fg_primary())
                                    .child("Broadcast Targets"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(theme::fg_muted())
                                    .child(format!("Rebroadcast mode: {scope_label}.")),
                            ),
                    )
                    .child(
                        div()
                            .h_flex()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .id("clipboard-broadcast-scope-all")
                                    .cursor_pointer()
                                    .hover(|this| this.bg(theme::bg_panel_alt()))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.request_clipboard_broadcast_scope(
                                            ClipboardBroadcastScope::AllConnected,
                                            cx,
                                        );
                                    }))
                                    .child(clipboard_tab_chip(
                                        "All",
                                        snapshot.broadcast_scope
                                            == ClipboardBroadcastScope::AllConnected,
                                        theme::accent_blue(),
                                    )),
                            )
                            .child(
                                div()
                                    .id("clipboard-broadcast-scope-selected")
                                    .cursor_pointer()
                                    .hover(|this| this.bg(theme::bg_panel_alt()))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.request_clipboard_broadcast_scope(
                                            ClipboardBroadcastScope::SelectedSessions,
                                            cx,
                                        );
                                    }))
                                    .child(clipboard_tab_chip(
                                        "Selected",
                                        snapshot.broadcast_scope
                                            == ClipboardBroadcastScope::SelectedSessions,
                                        theme::accent_cyan(),
                                    )),
                            ),
                    ),
            )
            .child(if snapshot.target_rows.is_empty() {
                div()
                    .text_size(px(12.0))
                    .text_color(theme::fg_muted())
                    .child("No active sessions are available for clipboard rebroadcast.")
                    .into_any_element()
            } else {
                div()
                    .h_flex()
                    .flex_wrap()
                    .gap(px(10.0))
                    .children(
                        snapshot
                            .target_rows
                            .iter()
                            .enumerate()
                            .map(|(index, target)| {
                                let accent = if target.selected {
                                    theme::accent_green()
                                } else {
                                    theme::accent_cyan()
                                };
                                let chip = div()
                                    .v_flex()
                                    .gap(px(4.0))
                                    .px(px(12.0))
                                    .py(px(10.0))
                                    .rounded(px(18.0))
                                    .bg(if target.selected {
                                        accent.opacity(0.12)
                                    } else {
                                        theme::bg_console()
                                    })
                                    .border_1()
                                    .border_color(if target.selected {
                                        accent.opacity(0.28)
                                    } else {
                                        theme::border_soft()
                                    })
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_semibold()
                                            .text_color(theme::fg_primary())
                                            .child(target.device_id.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .text_color(theme::fg_muted())
                                            .child(target.secondary_label.clone()),
                                    );

                                if target.interactive {
                                    div()
                                        .id(("clipboard-target", index))
                                        .cursor_pointer()
                                        .hover(|this| this.bg(theme::bg_panel_alt()))
                                        .on_click(cx.listener({
                                            let session_id = target.id;
                                            move |this, _, _, cx| {
                                                this.request_clipboard_toggle_session_target(
                                                    session_id, cx,
                                                );
                                            }
                                        }))
                                        .child(chip)
                                        .into_any_element()
                                } else {
                                    chip.opacity(0.78).into_any_element()
                                }
                            }),
                    )
                    .into_any_element()
            })
    }
}
