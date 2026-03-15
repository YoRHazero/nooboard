use gpui::Context;

use crate::{ui::workspace::WorkspaceView, workspace::actions::clipboard as clipboard_actions};

use super::super::state::ClipboardBroadcastScope;

impl WorkspaceView {
    pub(in crate::ui::workspace) fn request_clipboard_broadcast_scope(
        &mut self,
        scope: ClipboardBroadcastScope,
        cx: &mut Context<Self>,
    ) {
        self.clipboard.set_broadcast_scope(scope);
        cx.notify();
    }

    pub(in crate::ui::workspace) fn request_clipboard_toggle_session_target(
        &mut self,
        session_id: nooboard_core::SessionId,
        cx: &mut Context<Self>,
    ) {
        self.clipboard.toggle_session_target(session_id);
        cx.notify();
    }

    pub(in crate::ui::workspace) fn request_clipboard_adopt_selected(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let model = self.render_model(cx);
        let Some(record) = model.page.as_ref().and_then(|page| {
            self.clipboard
                .selected_record(page.clipboard.latest_record.as_ref())
        }) else {
            return;
        };
        if self.clipboard.adopt_in_flight_event_id() == Some(record.event_id) {
            return;
        }

        let Some(task) =
            clipboard_actions::adopt_clipboard_record_task(&self.controller, record.event_id, cx)
        else {
            return;
        };

        self.clipboard.start_adopt(record.event_id);
        cx.notify();

        let view = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        this.clipboard.finish_adopt(
                            record.event_id,
                            "Selected record adopted locally.".to_string(),
                        );
                    }
                    Err(error) => {
                        this.clipboard.finish_adopt(
                            record.event_id,
                            format!("Failed to adopt selected record: {error}"),
                        );
                    }
                }
                cx.notify();
            });
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }

    pub(in crate::ui::workspace) fn request_clipboard_rebroadcast_selected(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let model = self.render_model(cx);
        let Some(record) = model.page.as_ref().and_then(|page| {
            self.clipboard
                .selected_record(page.clipboard.latest_record.as_ref())
        }) else {
            return;
        };
        if self.clipboard.rebroadcast_in_flight_event_id() == Some(record.event_id) {
            return;
        }

        let target = self.clipboard.rebroadcast_target();
        let Some(task) = clipboard_actions::rebroadcast_clipboard_record_task(
            &self.controller,
            record.event_id,
            target,
            cx,
        ) else {
            return;
        };

        self.clipboard.start_rebroadcast(record.event_id);
        cx.notify();

        let view = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        this.clipboard.finish_rebroadcast(
                            record.event_id,
                            "Rebroadcast queued for the selected session target.".to_string(),
                        );
                    }
                    Err(error) => {
                        this.clipboard.finish_rebroadcast(
                            record.event_id,
                            format!("Failed to rebroadcast selected record: {error}"),
                        );
                    }
                }
                cx.notify();
            });
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }
}
