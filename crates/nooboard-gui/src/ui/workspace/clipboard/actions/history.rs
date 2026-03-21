use gpui::{Context, Window};
use nooboard_core::{
    ClipboardHistoryAnchor, ClipboardHistoryDirection, ClipboardHistoryPage, EventId,
    ListClipboardHistoryRequest,
};

use crate::{ui::workspace::WorkspaceView, workspace::actions::clipboard as clipboard_actions};

const CLIPBOARD_HISTORY_LIMIT: usize = 24;

impl WorkspaceView {
    pub(in crate::ui::workspace) fn reveal_pending_clipboard_history(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if self.clipboard.pending_new_count() == 0 {
            return;
        }

        self.clipboard.reveal_pending_new();
        cx.notify();
    }

    pub(in crate::ui::workspace) fn load_older_clipboard_history(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if !self.clipboard.can_load_older() {
            return;
        }

        let Some(anchor) = self.clipboard.older_anchor() else {
            return;
        };
        if !self
            .clipboard
            .begin_history_load(ClipboardHistoryDirection::Older, false)
        {
            return;
        }

        self.request_clipboard_history_page(ClipboardHistoryDirection::Older, Some(anchor), cx);
    }

    pub(in crate::ui::workspace) fn load_newer_clipboard_history(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if !self.clipboard.can_load_newer() {
            return;
        }

        let Some(anchor) = self.clipboard.newer_anchor() else {
            return;
        };
        if !self
            .clipboard
            .begin_history_load(ClipboardHistoryDirection::Newer, false)
        {
            return;
        }

        self.request_clipboard_history_page(ClipboardHistoryDirection::Newer, Some(anchor), cx);
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
                            .fail_history_load(format!("Couldn't load this clipboard item: {error}"));
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
        direction: ClipboardHistoryDirection,
        anchor: Option<ClipboardHistoryAnchor>,
        cx: &mut Context<Self>,
    ) {
        let Some(task) = clipboard_actions::list_clipboard_history_task(
            &self.controller,
            ListClipboardHistoryRequest {
                limit: CLIPBOARD_HISTORY_LIMIT,
                direction: direction.clone(),
                anchor,
            },
            cx,
        ) else {
            self.clipboard
                .fail_history_load("Clipboard history is still loading.".to_string());
            cx.notify();
            return;
        };

        let view = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok(page) => this.finish_clipboard_history_page(direction, page),
                    Err(error) => {
                        this.clipboard.fail_history_load(format!(
                            "Couldn't load clipboard history: {error}"
                        ));
                    }
                }
                cx.notify();
            });
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }

    fn finish_clipboard_history_page(
        &mut self,
        direction: ClipboardHistoryDirection,
        page: ClipboardHistoryPage,
    ) {
        let loaded_empty = page.records.is_empty();
        self.clipboard.finish_history_load(direction, page);
        if loaded_empty && self.clipboard.history_records().is_empty() {
            self.clipboard.fail_history_load(
                "No saved clipboard items yet.".to_string(),
            );
        }
    }
}
