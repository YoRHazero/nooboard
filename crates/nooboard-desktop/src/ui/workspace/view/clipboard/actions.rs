use std::collections::HashSet;

use anyhow::Error;
use gpui::{AppContext, Context, Styled};
use gpui_component::WindowExt;
use gpui_component::notification::Notification;
use gpui_component::{Icon, IconName};
use nooboard_app::{
    ClipboardBroadcastTargets, ListClipboardHistoryRequest, RebroadcastClipboardRequest,
    SubmitTextRequest,
};

use crate::state::live_commands;
use crate::ui::theme;

use super::WorkspaceView;
use super::page_state::{ClipboardBroadcastScope, ClipboardHistoryLoadState, ClipboardSelection};

const CLIPBOARD_HISTORY_PAGE_SIZE: usize = 24;

struct ClipboardAdoptFailureNotification;

impl WorkspaceView {
    pub(crate) fn bootstrap_clipboard_page(&mut self, cx: &mut Context<Self>) {
        if self.clipboard_page.history_bootstrapped {
            return;
        }

        self.clipboard_page.history_bootstrapped = true;
        self.load_clipboard_history_page(None, cx);
    }

    pub(crate) fn sync_clipboard_page_state(&mut self, cx: &mut Context<Self>) {
        let store = self.live_store.read(cx);
        let connected = store
            .app_state()
            .peers
            .connected
            .iter()
            .map(|peer| peer.noob_id.clone())
            .collect::<HashSet<_>>();
        self.clipboard_page.retain_connected_targets(&connected);

        if let Some(record) = store.latest_committed_record().cloned() {
            self.clipboard_page.cache_record(record.clone());
            if self.clipboard_page.latest_seen_committed_event_id != Some(record.event_id)
                || !self
                    .clipboard_page
                    .history_records
                    .iter()
                    .any(|existing| existing.event_id == record.event_id)
            {
                self.clipboard_page.promote_record(record);
            }
        }

        self.clipboard_page.latest_seen_committed_event_id =
            store.app_state().clipboard.latest_committed_event_id;
    }

    pub(crate) fn sync_clipboard_read_input(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let selected_record = self.selected_clipboard_record(cx);
        self.clipboard_page
            .sync_read_record(selected_record.as_ref(), window, cx);
    }

    pub(super) fn set_clipboard_feedback(&mut self, message: impl Into<String>) {
        self.clipboard_page.feedback = Some(message.into());
    }

    pub(super) fn set_clipboard_broadcast_scope(
        &mut self,
        scope: ClipboardBroadcastScope,
        cx: &mut Context<Self>,
    ) {
        if self.clipboard_page.broadcast_scope == scope {
            return;
        }

        self.clipboard_page.broadcast_scope = scope;
        cx.notify();
    }

    pub(super) fn toggle_clipboard_target(
        &mut self,
        noob_id: &nooboard_app::NoobId,
        cx: &mut Context<Self>,
    ) {
        if self.clipboard_page.broadcast_scope != ClipboardBroadcastScope::SelectedPeers {
            return;
        }

        if self
            .clipboard_page
            .selected_target_noob_ids
            .contains(noob_id)
        {
            self.clipboard_page.selected_target_noob_ids.remove(noob_id);
        } else {
            self.clipboard_page
                .selected_target_noob_ids
                .insert(noob_id.clone());
        }
        cx.notify();
    }

    pub(super) fn load_more_clipboard_history(&mut self, cx: &mut Context<Self>) {
        if !self.clipboard_page.can_load_more() {
            return;
        }

        self.load_clipboard_history_page(self.clipboard_page.next_cursor.clone(), cx);
    }

    pub(super) fn request_clipboard_adopt_locally(&mut self, cx: &mut Context<Self>) {
        let Some(record) = self.selected_clipboard_record(cx) else {
            return;
        };
        if self.clipboard_page.adopt_in_flight_event_id == Some(record.event_id) {
            return;
        }

        self.clipboard_page.adopt_in_flight_event_id = Some(record.event_id);
        self.set_clipboard_feedback("Adopting selected record locally.");
        cx.notify();

        let commands = live_commands::client(cx);
        let view = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = commands.adopt_clipboard_record_quiet(record.event_id).await;
            let _ = view.update(cx, |this, cx| {
                this.clipboard_page.adopt_in_flight_event_id = None;
                match result {
                    Ok(()) => {
                        this.set_clipboard_feedback("Selected record adopted locally.");
                    }
                    Err(error) => {
                        this.set_clipboard_feedback(format!(
                            "Failed to adopt clipboard record {}: {error}",
                            record.event_id
                        ));
                    }
                }
                cx.notify();
            });

            Ok::<_, Error>(())
        })
        .detach();
    }

    pub(super) fn request_clipboard_rebroadcast(&mut self, cx: &mut Context<Self>) {
        let snapshot = self.clipboard_snapshot(cx);
        let Some(record) = snapshot.selected_record else {
            return;
        };
        if self.clipboard_page.rebroadcast_in_flight_event_id == Some(record.event_id) {
            return;
        }

        let targets = match snapshot.broadcast_scope {
            ClipboardBroadcastScope::AllConnected => {
                if snapshot.connected_target_count == 0 {
                    self.set_clipboard_feedback(
                        "No connected peers are available for rebroadcast.",
                    );
                    cx.notify();
                    return;
                }
                ClipboardBroadcastTargets::AllConnected
            }
            ClipboardBroadcastScope::SelectedPeers => {
                if self.clipboard_page.selected_target_noob_ids.is_empty() {
                    self.set_clipboard_feedback(
                        "Select at least one connected peer to rebroadcast.",
                    );
                    cx.notify();
                    return;
                }
                ClipboardBroadcastTargets::Nodes(
                    self.clipboard_page
                        .selected_target_noob_ids
                        .iter()
                        .cloned()
                        .collect(),
                )
            }
        };

        self.clipboard_page.rebroadcast_in_flight_event_id = Some(record.event_id);
        self.set_clipboard_feedback("Rebroadcasting selected record to connected peers.");
        cx.notify();

        let commands = live_commands::client(cx);
        let view = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = commands
                .rebroadcast_clipboard_record(RebroadcastClipboardRequest {
                    event_id: record.event_id,
                    targets,
                })
                .await;
            let _ = view.update(cx, |this, cx| {
                this.clipboard_page.rebroadcast_in_flight_event_id = None;
                match result {
                    Ok(()) => {
                        this.set_clipboard_feedback(
                            "Rebroadcast queued. Waiting for connected peers to receive it.",
                        );
                    }
                    Err(error) => {
                        this.set_clipboard_feedback(format!(
                            "Failed to rebroadcast clipboard record {}: {error}",
                            record.event_id
                        ));
                    }
                }
                cx.notify();
            });

            Ok::<_, Error>(())
        })
        .detach();
    }

    pub(super) fn submit_clipboard_edit(&mut self, cx: &mut Context<Self>) {
        let max_text_bytes = self
            .live_store
            .read(cx)
            .app_state()
            .settings
            .storage
            .max_text_bytes;
        if !self.clipboard_page.can_submit_edit(max_text_bytes, cx) {
            return;
        }

        let edited_content = self.clipboard_page.edit_text(cx);
        self.clipboard_page.submit_in_flight = true;
        self.set_clipboard_feedback("Saving edited clipboard record.");
        cx.notify();

        let commands = live_commands::client(cx);
        let live_store = self.live_store.clone();
        let view = cx.entity().downgrade();
        let window_handle = self.window_handle.clone();
        cx.spawn(async move |_, cx| {
            let submit_result = commands
                .submit_text(SubmitTextRequest {
                    content: edited_content,
                })
                .await;

            match submit_result {
                Ok(new_event_id) => {
                    let maybe_record = commands.get_clipboard_record(new_event_id).await.ok();
                    let adopt_failure_message = commands
                        .adopt_clipboard_record_quiet(new_event_id)
                        .await
                        .err()
                        .map(|error| error.to_string());
                    let _ = cx.update_window(window_handle, |_, window, cx| {
                        if let Some(view) = view.upgrade() {
                            view.update(cx, |this, cx| {
                                this.clipboard_page.submit_in_flight = false;
                                this.clipboard_page.selection = ClipboardSelection::Pinned(new_event_id);
                                if let Some(record) = maybe_record.clone() {
                                    this.clipboard_page.promote_record(record);
                                }
                                this.clipboard_page.clear_edit_session(window, cx);
                                if adopt_failure_message.is_some() {
                                    this.set_clipboard_feedback(
                                        "Edited record saved, but local adopt failed.",
                                    );
                                } else if maybe_record.is_some() {
                                    this.set_clipboard_feedback(
                                        "Edited record saved and adopted locally.",
                                    );
                                } else {
                                    this.set_clipboard_feedback(
                                        "Edited record saved. Waiting for app state to provide the new record.",
                                    );
                                }
                                cx.notify();
                            });
                        }

                        if let Some(message) = adopt_failure_message {
                            window.push_notification(
                                Notification::warning(
                                    "Edited clipboard record was saved, but local adopt failed.",
                                )
                                .title("Clipboard saved, local adopt failed")
                                .icon(
                                    Icon::new(IconName::TriangleAlert)
                                        .text_color(theme::accent_amber()),
                                )
                                .bg(theme::bg_panel())
                                .border_color(theme::border_base())
                                .text_color(theme::fg_primary())
                                .id1::<ClipboardAdoptFailureNotification>(new_event_id.to_string()),
                                cx,
                            );
                            let _ = live_store.update(cx, |store, cx| {
                                store.record_clipboard_adopt_failed(new_event_id, message);
                                cx.notify();
                            });
                        }
                    });
                }
                Err(error) => {
                    let _ = view.update(cx, |this, cx| {
                        this.clipboard_page.submit_in_flight = false;
                        this.set_clipboard_feedback(format!(
                            "Failed to save the edited clipboard record: {error}"
                        ));
                        cx.notify();
                    });
                }
            }

            Ok::<_, Error>(())
        })
        .detach();
    }

    fn load_clipboard_history_page(
        &mut self,
        cursor: Option<nooboard_app::ClipboardHistoryCursor>,
        cx: &mut Context<Self>,
    ) {
        let is_initial = cursor.is_none();
        self.clipboard_page.history_load_state = if is_initial {
            ClipboardHistoryLoadState::LoadingInitial
        } else {
            ClipboardHistoryLoadState::LoadingMore
        };
        cx.notify();

        let commands = live_commands::client(cx);
        let view = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = commands
                .list_clipboard_history(ListClipboardHistoryRequest {
                    limit: CLIPBOARD_HISTORY_PAGE_SIZE,
                    cursor,
                })
                .await;
            let _ = view.update(cx, |this, cx| {
                this.clipboard_page.history_load_state = ClipboardHistoryLoadState::Idle;
                match result {
                    Ok(page) => {
                        this.clipboard_page
                            .append_history_page(page.records, page.next_cursor);
                        if is_initial && this.clipboard_page.history_records.is_empty() {
                            this.set_clipboard_feedback(
                                "No committed clipboard records have been stored yet.",
                            );
                        }
                    }
                    Err(error) => {
                        this.set_clipboard_feedback(format!(
                            "Failed to load clipboard history: {error}"
                        ));
                    }
                }
                cx.notify();
            });

            Ok::<_, Error>(())
        })
        .detach();
    }
}
