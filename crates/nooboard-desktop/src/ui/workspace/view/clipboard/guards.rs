use gpui::{Context, ParentElement, Styled, Window, div, px};
use gpui_component::StyledExt;
use gpui_component::WindowExt;
use gpui_component::button::ButtonVariant;
use gpui_component::dialog::DialogButtonProps;

use crate::state::WorkspaceRoute;
use crate::ui::theme;

use super::WorkspaceView;
use super::page_state::{ClipboardDetailTab, ClipboardExitIntent, ClipboardSelection};

impl WorkspaceView {
    pub(crate) fn request_workspace_route(
        &mut self,
        route: WorkspaceRoute,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.route == route {
            return;
        }
        if self.request_clipboard_exit(ClipboardExitIntent::NavigateRoute(route), window, cx) {
            return;
        }

        if self.route == WorkspaceRoute::Clipboard {
            self.apply_clipboard_exit_intent(ClipboardExitIntent::NavigateRoute(route), window, cx);
        } else {
            self.route = route;
        }
        cx.notify();
    }

    pub(super) fn request_clipboard_detail_tab(
        &mut self,
        tab: ClipboardDetailTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.clipboard_page.detail_tab == tab {
            return;
        }

        match tab {
            ClipboardDetailTab::Read => {
                if self.request_clipboard_exit(ClipboardExitIntent::SwitchTab(tab), window, cx) {
                    return;
                }
                self.apply_clipboard_exit_intent(ClipboardExitIntent::SwitchTab(tab), window, cx);
            }
            ClipboardDetailTab::Edit => {
                let Some(record) = self.selected_clipboard_record(cx) else {
                    return;
                };
                self.clipboard_page.begin_edit_session(&record, window, cx);
                cx.notify();
            }
        }
    }

    pub(super) fn request_clipboard_select_latest(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.clipboard_page.selection == ClipboardSelection::LatestCommitted {
            return;
        }
        if self.request_clipboard_exit(ClipboardExitIntent::SelectLatest, window, cx) {
            return;
        }

        self.apply_clipboard_exit_intent(ClipboardExitIntent::SelectLatest, window, cx);
    }

    pub(super) fn request_clipboard_select_history(
        &mut self,
        event_id: nooboard_app::EventId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.clipboard_page.selection == ClipboardSelection::Pinned(event_id) {
            return;
        }
        if self.request_clipboard_exit(ClipboardExitIntent::SelectHistory(event_id), window, cx) {
            return;
        }

        self.apply_clipboard_exit_intent(ClipboardExitIntent::SelectHistory(event_id), window, cx);
    }

    fn request_clipboard_exit(
        &mut self,
        intent: ClipboardExitIntent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.route != WorkspaceRoute::Clipboard
            || self.clipboard_page.detail_tab != ClipboardDetailTab::Edit
        {
            return false;
        }

        if self.clipboard_page.submit_in_flight {
            self.set_clipboard_feedback("Wait for the current clipboard save to finish.");
            cx.notify();
            return true;
        }

        if !self.clipboard_page.is_edit_dirty(cx) {
            return false;
        }

        if self.clipboard_page.discard_confirm_open {
            return true;
        }

        self.clipboard_page.discard_confirm_open = true;
        let view = cx.entity().downgrade();
        let cancel_view = view.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let ok_view = view.clone();
            let cancel_view = cancel_view.clone();
            dialog
                .title("Discard edited clipboard draft?")
                .button_props(
                    DialogButtonProps::default()
                        .show_cancel(true)
                        .ok_text("Discard")
                        .ok_variant(ButtonVariant::Warning)
                        .cancel_text("Stay"),
                )
                .overlay_closable(false)
                .close_button(false)
                .child(
                    div()
                        .v_flex()
                        .gap(px(10.0))
                        .child(
                            div()
                                .text_size(px(14.0))
                                .text_color(theme::fg_primary())
                                .child(
                                    "You have unsaved changes in the Edit tab. Leaving this context will discard them."
                                        .to_string(),
                                ),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(theme::fg_muted())
                                .child("Discard the current edits and continue?".to_string()),
                        ),
                )
                .on_ok(move |_, window, cx| {
                    if let Some(view) = ok_view.upgrade() {
                        view.update(cx, |this, cx| {
                            this.clipboard_page.discard_confirm_open = false;
                            this.apply_clipboard_exit_intent(intent, window, cx);
                            cx.notify();
                        });
                    }
                    true
                })
                .on_cancel(move |_, _, cx| {
                    if let Some(view) = cancel_view.upgrade() {
                        view.update(cx, |this, cx| {
                            this.clipboard_page.discard_confirm_open = false;
                            cx.notify();
                        });
                    }
                    true
                })
        });
        cx.notify();
        true
    }

    fn apply_clipboard_exit_intent(
        &mut self,
        intent: ClipboardExitIntent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.clipboard_page.detail_tab == ClipboardDetailTab::Edit {
            self.clipboard_page.clear_edit_session(window, cx);
        }

        match intent {
            ClipboardExitIntent::SelectLatest => {
                self.clipboard_page.selection = ClipboardSelection::LatestCommitted;
            }
            ClipboardExitIntent::SelectHistory(event_id) => {
                self.clipboard_page.selection = ClipboardSelection::Pinned(event_id);
            }
            ClipboardExitIntent::SwitchTab(tab) => {
                self.clipboard_page.detail_tab = tab;
            }
            ClipboardExitIntent::NavigateRoute(route) => {
                self.route = route;
            }
        }
    }
}
