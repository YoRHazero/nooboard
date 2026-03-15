use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    div, px,
};
use gpui_component::input::Input;
use gpui_component::{Sizable, StyledExt};

use crate::ui::theme;

use super::{
    CLIPBOARD_DETAIL_MIN_WIDTH, CLIPBOARD_PANEL_MIN_HEIGHT,
    components::{
        clipboard_badge, clipboard_panel_shell, clipboard_placeholder_copy, clipboard_tab_chip,
    },
    state::{ClipboardBroadcastScope, ClipboardDetailTab},
    view_state::{
        ClipboardPageViewState, clipboard_record_time_label, clipboard_short_event_id,
        clipboard_source_label,
    },
};
use crate::ui::workspace::WorkspaceView;

impl WorkspaceView {
    pub(super) fn clipboard_detail_panel(
        &self,
        snapshot: &ClipboardPageViewState,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex_1()
            .min_w(px(CLIPBOARD_DETAIL_MIN_WIDTH))
            .v_flex()
            .gap(px(16.0))
            .child(self.clipboard_info_panel(snapshot, cx))
            .child(self.clipboard_content_panel(snapshot, cx))
    }

    fn clipboard_info_panel(
        &self,
        snapshot: &ClipboardPageViewState,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        {
            let mut panel =
                clipboard_panel_shell()
                    .v_flex()
                    .gap(px(12.0))
                    .p(px(16.0))
                    .child(
                        div()
                            .h_flex()
                            .items_start()
                            .justify_between()
                            .gap(px(14.0))
                            .child({
                                let info = div().v_flex().gap(px(8.0)).flex_1().min_w(px(0.0));
                                if let Some(record) = snapshot.selected_record.clone() {
                                    let accent = self.clipboard_source_accent(record.source);
                                    info.child(
                                        div()
                                            .h_flex()
                                            .items_center()
                                            .gap(px(8.0))
                                            .flex_wrap()
                                            .child(clipboard_badge(
                                                clipboard_source_label(record.source),
                                                accent,
                                            ))
                                            .child(clipboard_badge(
                                                if snapshot.latest_selected {
                                                    "Latest committed"
                                                } else {
                                                    "Pinned record"
                                                },
                                                theme::accent_amber(),
                                            )),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(15.0))
                                            .font_semibold()
                                            .text_color(theme::fg_primary())
                                            .truncate()
                                            .child(record.origin_device_id.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(theme::fg_muted())
                                            .truncate()
                                            .child(format!(
                                                "{} · event {}",
                                                clipboard_record_time_label(&record),
                                                clipboard_short_event_id(&record)
                                            )),
                                    )
                                } else {
                                    info.child(
                                div()
                                    .text_size(px(14.0))
                                    .font_semibold()
                                    .text_color(theme::fg_primary())
                                    .child("No committed clipboard record selected"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(theme::fg_muted())
                                    .child("Load history or wait for the next committed record."),
                            )
                                }
                            })
                            .child(self.clipboard_detail_tabs(snapshot, cx)),
                    );

            if let Some(message) = snapshot.feedback.clone() {
                panel = panel.child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme::fg_muted())
                        .line_clamp(2)
                        .text_ellipsis()
                        .child(message),
                );
            }

            panel
        }
    }

    fn clipboard_detail_tabs(
        &self,
        snapshot: &ClipboardPageViewState,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        div()
            .h_flex()
            .gap(px(8.0))
            .child(self.clipboard_detail_tab(
                "Read",
                snapshot.detail_tab == ClipboardDetailTab::Read,
                theme::accent_blue(),
                ClipboardDetailTab::Read,
                true,
                cx,
            ))
            .child(self.clipboard_detail_tab(
                "Edit",
                snapshot.detail_tab == ClipboardDetailTab::Edit,
                theme::accent_cyan(),
                ClipboardDetailTab::Edit,
                snapshot.can_enter_edit,
                cx,
            ))
    }

    fn clipboard_detail_tab(
        &self,
        label: &'static str,
        selected: bool,
        accent: gpui::Hsla,
        tab: ClipboardDetailTab,
        enabled: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let chip = clipboard_tab_chip(label, selected, accent);
        if enabled {
            div()
                .id(format!("clipboard-detail-tab-{label}"))
                .cursor_pointer()
                .hover(|this| this.bg(theme::bg_panel_alt()))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.request_clipboard_detail_tab(tab, window, cx);
                }))
                .child(chip)
                .into_any_element()
        } else {
            chip.opacity(0.46).into_any_element()
        }
    }

    fn clipboard_content_panel(
        &self,
        snapshot: &ClipboardPageViewState,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let Some(_record) = snapshot.selected_record.clone() else {
            return clipboard_panel_shell()
                .flex_1()
                .min_h(px(CLIPBOARD_PANEL_MIN_HEIGHT))
                .child(clipboard_placeholder_copy(
                    "Clipboard detail",
                    "Committed records will appear here once they are loaded.",
                ));
        };

        let can_rebroadcast = match snapshot.broadcast_scope {
            ClipboardBroadcastScope::AllConnected => snapshot.connected_target_count > 0,
            ClipboardBroadcastScope::SelectedSessions => snapshot.selected_target_count > 0,
        };

        clipboard_panel_shell()
            .flex_1()
            .min_h(px(CLIPBOARD_PANEL_MIN_HEIGHT))
            .v_flex()
            .gap(px(14.0))
            .p(px(22.0))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(18.0))
                            .font_semibold()
                            .text_color(theme::fg_primary())
                            .child(match snapshot.detail_tab {
                                ClipboardDetailTab::Read => "Content",
                                ClipboardDetailTab::Edit => "Edit",
                            }),
                    )
                    .child(match snapshot.detail_tab {
                        ClipboardDetailTab::Read => self
                            .clipboard_read_actions(can_rebroadcast, snapshot, cx)
                            .into_any_element(),
                        ClipboardDetailTab::Edit => {
                            self.clipboard_edit_actions(snapshot, cx).into_any_element()
                        }
                    }),
            )
            .child(match snapshot.detail_tab {
                ClipboardDetailTab::Read => self.clipboard_read_content().into_any_element(),
                ClipboardDetailTab::Edit => {
                    self.clipboard_edit_content(snapshot).into_any_element()
                }
            })
    }

    fn clipboard_read_content(&self) -> impl IntoElement {
        div()
            .flex_1()
            .w_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .v_flex()
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .min_w(px(0.0))
                    .min_h(px(0.0))
                    .px(px(14.0))
                    .py(px(12.0))
                    .bg(theme::bg_console())
                    .border_1()
                    .border_color(theme::border_soft())
                    .rounded(px(18.0))
                    .overflow_hidden()
                    .child(
                        Input::new(&self.clipboard.read_input())
                            .small()
                            .h_full()
                            .w_full()
                            .appearance(false)
                            .bordered(false)
                            .focus_bordered(false)
                            .disabled(true)
                            .text_color(theme::fg_primary())
                            .flex_1(),
                    ),
            )
    }

    fn clipboard_edit_content(&self, snapshot: &ClipboardPageViewState) -> impl IntoElement {
        div()
            .flex_1()
            .w_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .v_flex()
            .gap(px(10.0))
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .min_w(px(0.0))
                    .min_h(px(0.0))
                    .px(px(14.0))
                    .py(px(12.0))
                    .bg(theme::bg_console())
                    .border_1()
                    .border_color(theme::border_soft())
                    .rounded(px(18.0))
                    .overflow_hidden()
                    .child(
                        Input::new(&self.clipboard.edit_input())
                            .small()
                            .h_full()
                            .w_full()
                            .appearance(false)
                            .bordered(false)
                            .focus_bordered(false)
                            .text_color(theme::fg_primary())
                            .flex_1(),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme::fg_muted())
                            .child(
                                "Saving creates a new record. nooboard-core then updates the workspace snapshot.",
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_semibold()
                            .text_color(if snapshot.edit_bytes > snapshot.max_text_bytes {
                                theme::accent_rose()
                            } else if snapshot.edit_dirty {
                                theme::accent_cyan()
                            } else {
                                theme::fg_muted()
                            })
                            .child(format!(
                                "{} / {} bytes",
                                snapshot.edit_bytes, snapshot.max_text_bytes
                            )),
                    ),
            )
    }

    fn clipboard_read_actions(
        &self,
        can_rebroadcast: bool,
        snapshot: &ClipboardPageViewState,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        div()
            .h_flex()
            .gap(px(8.0))
            .child(self.toolbar_button(
                "clipboard-action-adopt",
                if snapshot.adopt_in_flight {
                    "Adopting"
                } else {
                    "Adopt"
                },
                snapshot.selected_record.is_some() && !snapshot.adopt_in_flight,
                theme::accent_cyan(),
                |this, _, _, cx| this.request_clipboard_adopt_selected(cx),
                cx,
            ))
            .child(self.toolbar_button(
                "clipboard-action-rebroadcast",
                if snapshot.rebroadcast_in_flight {
                    "Sending"
                } else {
                    "Rebroadcast"
                },
                can_rebroadcast
                    && snapshot.selected_record.is_some()
                    && !snapshot.rebroadcast_in_flight,
                theme::accent_green(),
                |this, _, _, cx| this.request_clipboard_rebroadcast_selected(cx),
                cx,
            ))
            .child(self.toolbar_button(
                "clipboard-action-edit",
                "Edit",
                snapshot.can_enter_edit,
                theme::accent_amber(),
                |this, _, window, cx| {
                    this.request_clipboard_detail_tab(ClipboardDetailTab::Edit, window, cx);
                },
                cx,
            ))
    }

    fn clipboard_edit_actions(
        &self,
        snapshot: &ClipboardPageViewState,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        div()
            .h_flex()
            .gap(px(8.0))
            .child(self.toolbar_button(
                "clipboard-action-save",
                if snapshot.submit_in_flight {
                    "Saving"
                } else {
                    "Save As New"
                },
                snapshot.can_submit_edit && !snapshot.submit_in_flight,
                theme::accent_green(),
                |this, _, window, cx| this.request_clipboard_submit_edit(window, cx),
                cx,
            ))
            .child(self.toolbar_button(
                "clipboard-action-cancel",
                "Cancel",
                true,
                theme::accent_rose(),
                |this, _, window, cx| this.request_clipboard_cancel_edit(window, cx),
                cx,
            ))
    }
}
