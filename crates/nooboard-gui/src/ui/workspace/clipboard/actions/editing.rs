use gpui::{AppContext as _, Context, Window};

use crate::{ui::workspace::WorkspaceView, workspace::actions::clipboard as clipboard_actions};

use super::super::state::ClipboardDetailTab;

impl WorkspaceView {
    pub(in crate::ui::workspace) fn request_clipboard_detail_tab(
        &mut self,
        tab: ClipboardDetailTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match tab {
            ClipboardDetailTab::Read => self.clipboard.clear_edit_session(window, cx),
            ClipboardDetailTab::Edit => {
                let model = self.render_model(cx);
                let selected_record = model.page.as_ref().and_then(|page| {
                    self.clipboard
                        .selected_record(page.clipboard.latest_record.as_ref())
                });
                let Some(record) = selected_record else {
                    return;
                };
                self.clipboard.begin_edit_session(&record, window, cx);
            }
        }
        cx.notify();
    }

    pub(in crate::ui::workspace) fn request_clipboard_cancel_edit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.clipboard.clear_edit_session(window, cx);
        cx.notify();
    }

    pub(in crate::ui::workspace) fn request_clipboard_submit_edit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let model = self.render_model(cx);
        let Some(page) = model.page.as_ref() else {
            return;
        };
        if !self
            .clipboard
            .can_submit_edit(page.clipboard.max_text_bytes, cx)
        {
            return;
        }

        let content = self.clipboard.edit_text(cx);
        let Some(task) = clipboard_actions::submit_text_task(&self.controller, content, cx) else {
            return;
        };
        self.clipboard.start_submit();
        cx.notify();

        let view = cx.entity().downgrade();
        let window_handle = window.window_handle();
        cx.spawn(async move |_, cx| {
            let result = task.await;
            let _ = cx.update_window(window_handle, |_, window, cx| {
                if let Some(view) = view.upgrade() {
                    let _ = view.update(cx, |this, cx| {
                        match result {
                            Ok(_) => {
                                this.clipboard.finish_submit(
                                    window,
                                    cx,
                                    "Edited record saved. Waiting for the latest committed snapshot update.".to_string(),
                                );
                            }
                            Err(error) => {
                                this.clipboard.fail_submit(format!(
                                    "Failed to save edited clipboard content: {error}"
                                ));
                            }
                        }
                        cx.notify();
                    });
                }
            });
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }
}
