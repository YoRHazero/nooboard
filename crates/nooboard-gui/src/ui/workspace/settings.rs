use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px};
use gpui_component::StyledExt;

use super::{WorkspaceRenderModel, WorkspaceView};

impl WorkspaceView {
    pub(super) fn settings_page(&self, model: &WorkspaceRenderModel) -> Vec<AnyElement> {
        let settings_rows = model
            .page
            .as_ref()
            .map(|state| state.settings_rows.clone())
            .unwrap_or_default();

        vec![
            div()
                .h_flex()
                .flex_wrap()
                .gap(px(12.0))
                .children(settings_rows.iter().map(|metric| self.metric_card(metric)))
                .into_any_element(),
        ]
    }
}
