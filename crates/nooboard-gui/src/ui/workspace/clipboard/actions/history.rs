use gpui::{Context, Window};
use nooboard_core::{
    ClipboardHistoryCursor, ClipboardHistoryPage, EventId, ListClipboardHistoryRequest,
};

use crate::{ui::workspace::WorkspaceView, workspace::actions::clipboard as clipboard_actions};

const CLIPBOARD_HISTORY_LIMIT: usize = 24;

impl WorkspaceView {
    pub(in crate::ui::workspace) fn load_more_clipboard_history(&mut self, cx: &mut Context<Self>) {
        if !self.clipboard.can_load_more() {
            return;
        }

        let Some(cursor) = self.clipboard.next_cursor() else {
            return;
        };
        if !self.clipboard.begin_history_load(false) {
            return;
        }

        self.request_clipboard_history_page(Some(cursor), cx);
    }

    pub(in crate::ui::workspace) fn request_clipboard_select_latest(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.clipboard.select_latest();
        self.sync_clipboard_from_workspace(
            self.render_model(cx)
                .page
                .as_ref()
                .map(|page| &page.clipboard),
            window,
            cx,
        );
        cx.notify();
    }

    pub(in crate::ui::workspace) fn request_clipboard_select_history(
        &mut self,
        event_id: EventId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.clipboard.select_history(event_id);
        self.sync_clipboard_from_workspace(
            self.render_model(cx)
                .page
                .as_ref()
                .map(|page| &page.clipboard),
            window,
            cx,
        );
        if self.clipboard.has_cached_record(event_id) {
            cx.notify();
            return;
        }

        let Some(task) =
            clipboard_actions::get_clipboard_record_task(&self.controller, event_id, cx)
        else {
            cx.notify();
            return;
        };
        let view = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok(record) => this.clipboard.cache_record(record),
                    Err(error) => {
                        this.clipboard
                            .fail_history_load(format!("Failed to load clipboard record: {error}"));
                    }
                }
                cx.notify();
            });
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }

    pub(super) fn request_clipboard_history_page(
        &mut self,
        cursor: Option<ClipboardHistoryCursor>,
        cx: &mut Context<Self>,
    ) {
        let Some(task) = clipboard_actions::list_clipboard_history_task(
            &self.controller,
            ListClipboardHistoryRequest {
                limit: CLIPBOARD_HISTORY_LIMIT,
                cursor,
            },
            cx,
        ) else {
            self.clipboard
                .fail_history_load("Clipboard core bridge is not ready yet.".to_string());
            cx.notify();
            return;
        };

        let view = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok(page) => this.finish_clipboard_history_page(page),
                    Err(error) => {
                        this.clipboard.fail_history_load(format!(
                            "Failed to load clipboard history: {error}"
                        ));
                    }
                }
                cx.notify();
            });
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }

    fn finish_clipboard_history_page(&mut self, page: ClipboardHistoryPage) {
        let loaded_empty = page.records.is_empty();
        self.clipboard.finish_history_load(page);
        if loaded_empty && self.clipboard.history_records().is_empty() {
            self.clipboard.fail_history_load(
                "No committed clipboard records have been stored yet.".to_string(),
            );
        }
    }
}
