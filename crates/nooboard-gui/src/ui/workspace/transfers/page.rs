use gpui::{AnyElement, Context, IntoElement};

use super::super::{WorkspaceRenderModel, WorkspaceView};

impl WorkspaceView {
    pub(in crate::ui::workspace) fn transfers_page(
        &self,
        model: &WorkspaceRenderModel,
        cx: &Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(state) = model.page.as_ref().map(|page| &page.transfers) else {
            return vec![
                self.list_card(
                    "Transfers",
                    &["Waiting for workspace snapshot.".to_string()],
                    "Waiting for workspace snapshot.",
                )
                .into_any_element(),
            ];
        };

        vec![
            self.transfers_target_panel(state, cx).into_any_element(),
            self.transfers_upload_panel(cx).into_any_element(),
            self.transfers_activity_panel(state, cx).into_any_element(),
        ]
    }
}
