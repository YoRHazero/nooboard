use gpui::{AnyElement, Context, IntoElement};

use super::super::{WorkspaceRenderModel, WorkspaceView};

impl WorkspaceView {
    pub(in crate::ui::workspace) fn settings_page(
        &self,
        model: &WorkspaceRenderModel,
        cx: &Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(state) = model.page.as_ref().map(|page| &page.settings) else {
            return vec![
                self.list_card(
                    "Settings",
                    &["Loading your settings.".to_string()],
                    "Loading your settings.",
                )
                .into_any_element(),
            ];
        };

        let connection_dirty = self.connection_settings_dirty(state, cx);
        let clipboard_dirty = self.clipboard_settings_dirty(state);
        let transfers_dirty = self.transfer_settings_dirty(state, cx);
        let storage_dirty = self.storage_settings_dirty(state, cx);
        let any_dirty = connection_dirty || clipboard_dirty || transfers_dirty || storage_dirty;
        let (banner_label, banner_accent) =
            self.settings_banner_status(any_dirty, self.settings.applying());

        vec![
            self.settings_feedback_banner(
                banner_label,
                banner_accent,
                self.settings.feedback().cloned().unwrap_or_else(|| {
                    if any_dirty {
                        "You have unsaved changes on this page.".to_string()
                    } else {
                        "These settings show what nooboard is using right now.".to_string()
                    }
                }),
            )
            .into_any_element(),
            self.connection_settings_panel(state, connection_dirty, cx)
                .into_any_element(),
            self.clipboard_settings_panel(state, clipboard_dirty, cx)
                .into_any_element(),
            self.transfer_settings_panel(state, transfers_dirty, cx)
                .into_any_element(),
            self.storage_settings_panel(state, storage_dirty, cx)
                .into_any_element(),
            self.advanced_settings_panel(model, cx).into_any_element(),
        ]
    }
}
