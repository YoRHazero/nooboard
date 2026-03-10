use gpui::prelude::FluentBuilder as _;
use gpui::{AnyElement, Hsla, InteractiveElement, StatefulInteractiveElement};
use gpui_component::IconName;
use gpui_component::input::Input;
use gpui_component::{Sizable, StyledExt};
use nooboard_app::ClipboardRecord;

use super::components::{clipboard_icon_action_button, clipboard_mode_tab};
use super::page_state::{ClipboardBroadcastScope, ClipboardDetailTab};
use super::snapshot::{clipboard_record_time_label, clipboard_source_label};
use super::*;

impl WorkspaceView {
    pub(super) fn clipboard_detail_panel(
        &self,
        snapshot: &ClipboardSnapshot,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .flex_1()
            .min_w(px(0.0))
            .v_flex()
            .gap(px(16.0))
            .child(self.clipboard_info_panel(snapshot, cx))
            .child(self.clipboard_content_panel(snapshot, cx))
    }

    fn clipboard_info_panel(&self, snapshot: &ClipboardSnapshot, cx: &mut Context<Self>) -> Div {
        let feedback = snapshot.feedback.clone();

        clipboard_panel_shell()
            .rounded(px(20.0))
            .v_flex()
            .gap(px(12.0))
            .p(px(16.0))
            .child(
                div()
                    .h_flex()
                    .items_start()
                    .justify_between()
                    .gap(px(14.0))
                    .child(
                        div()
                            .v_flex()
                            .gap(px(8.0))
                            .flex_1()
                            .min_w(px(0.0))
                            .when_some(snapshot.selected_record.clone(), |this, record| {
                                let accent = self.clipboard_source_accent(record.source);
                                this.child(
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
                                            self.clipboard_short_event_id(record.event_id)
                                        )),
                                )
                            })
                            .when(snapshot.selected_record.is_none(), |this| {
                                this.child(
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
                                        .child(
                                            "Load history or wait for the next committed record.",
                                        ),
                                )
                            }),
                    )
                    .child(self.clipboard_detail_tabs(snapshot, cx)),
            )
            .when_some(feedback, |this, message| {
                this.child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme::fg_muted())
                        .line_clamp(2)
                        .text_ellipsis()
                        .child(message),
                )
            })
    }

    fn clipboard_detail_tabs(&self, snapshot: &ClipboardSnapshot, cx: &mut Context<Self>) -> Div {
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
        accent: Hsla,
        tab: ClipboardDetailTab,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tab_chip = clipboard_mode_tab(label, selected, accent);
        if enabled {
            tab_chip
                .id(format!("clipboard-detail-tab-{label}"))
                .cursor_pointer()
                .hover(|this| {
                    this.bg(theme::bg_panel_alt())
                        .border_color(theme::border_strong())
                })
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.request_clipboard_detail_tab(tab, window, cx);
                }))
                .into_any_element()
        } else {
            tab_chip.opacity(0.46).into_any_element()
        }
    }

    fn clipboard_content_panel(&self, snapshot: &ClipboardSnapshot, cx: &mut Context<Self>) -> Div {
        let Some(record) = snapshot.selected_record.clone() else {
            return clipboard_panel_shell()
                .rounded(px(24.0))
                .flex_1()
                .min_h(px(CLIPBOARD_TEXT_PANEL_MIN_HEIGHT))
                .v_flex()
                .items_center()
                .justify_center()
                .gap(px(10.0))
                .p(px(24.0))
                .child(
                    div()
                        .text_size(px(18.0))
                        .font_semibold()
                        .text_color(theme::fg_primary())
                        .child("Clipboard detail"),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(theme::fg_muted())
                        .child("Committed records will appear here once they are loaded."),
                );
        };

        let can_rebroadcast = match snapshot.broadcast_scope {
            ClipboardBroadcastScope::AllConnected => snapshot.connected_target_count > 0,
            ClipboardBroadcastScope::SelectedPeers => snapshot.selected_target_count > 0,
        };

        clipboard_panel_shell()
            .rounded(px(24.0))
            .flex_1()
            .min_h(px(CLIPBOARD_TEXT_PANEL_MIN_HEIGHT))
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
                ClipboardDetailTab::Read => self.clipboard_read_content(record).into_any_element(),
                ClipboardDetailTab::Edit => {
                    self.clipboard_edit_content(snapshot).into_any_element()
                }
            })
    }

    fn clipboard_read_content(&self, record: ClipboardRecord) -> AnyElement {
        div()
            .flex_1()
            .min_h(px(0.0))
            .overflow_y_scrollbar()
            .child(
                div()
                    .w_full()
                    .p(px(18.0))
                    .bg(theme::bg_console())
                    .border_1()
                    .border_color(theme::border_soft())
                    .rounded(px(18.0))
                    .text_size(px(14.0))
                    .text_color(theme::fg_primary())
                    .whitespace_normal()
                    .child(record.content),
            )
            .into_any_element()
    }

    fn clipboard_edit_content(&self, snapshot: &ClipboardSnapshot) -> Div {
        div()
            .flex_1()
            .min_h(px(0.0))
            .v_flex()
            .gap(px(10.0))
            .child(
                Input::new(&self.clipboard_page.edit_input)
                    .small()
                    .w_full()
                    .flex_1(),
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
                                "Saving creates a new record, then adopts it locally through app.",
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
        snapshot: &ClipboardSnapshot,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .h_flex()
            .gap(px(8.0))
            .child(
                clipboard_icon_action_button(
                    "clipboard-action-adopt",
                    IconName::Copy,
                    "Adopt this committed record locally through app",
                    theme::accent_blue(),
                    snapshot.adopt_in_flight,
                    cx,
                )
                .loading(snapshot.adopt_in_flight)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.request_clipboard_adopt_locally(cx);
                })),
            )
            .child(
                clipboard_icon_action_button(
                    "clipboard-action-rebroadcast",
                    IconName::ArrowUp,
                    "Rebroadcast this committed record to connected peers",
                    theme::accent_cyan(),
                    snapshot.rebroadcast_in_flight || !can_rebroadcast,
                    cx,
                )
                .loading(snapshot.rebroadcast_in_flight)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.request_clipboard_rebroadcast(cx);
                })),
            )
    }

    fn clipboard_edit_actions(&self, snapshot: &ClipboardSnapshot, cx: &mut Context<Self>) -> Div {
        div()
            .h_flex()
            .gap(px(8.0))
            .child(
                clipboard_icon_action_button(
                    "clipboard-action-submit-edit",
                    IconName::Check,
                    "Save the edited content as a new record and adopt it locally",
                    theme::accent_cyan(),
                    !snapshot.can_submit_edit,
                    cx,
                )
                .loading(snapshot.submit_in_flight)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.submit_clipboard_edit(cx);
                })),
            )
            .child(
                clipboard_icon_action_button(
                    "clipboard-action-cancel-edit",
                    IconName::Close,
                    "Leave Edit and return to Read",
                    theme::accent_rose(),
                    snapshot.submit_in_flight,
                    cx,
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.request_clipboard_detail_tab(ClipboardDetailTab::Read, window, cx);
                })),
            )
    }
}
