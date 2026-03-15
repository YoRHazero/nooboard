use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px};
use gpui_component::StyledExt;

use crate::ui::theme;

use super::{WorkspaceRenderModel, WorkspaceView};

impl WorkspaceView {
    pub(super) fn clipboard_page(
        &self,
        model: &WorkspaceRenderModel,
        cx: &Context<Self>,
    ) -> Vec<AnyElement> {
        let state = model.page.as_ref();

        vec![
            div()
                .h_flex()
                .gap(px(10.0))
                .child(self.toolbar_button(
                    "clipboard-adopt-latest",
                    "Adopt Latest",
                    state.is_some_and(|state| state.can_adopt_latest),
                    theme::accent_green(),
                    |this, _, _, cx| this.adopt_latest_clipboard_action(cx),
                    cx,
                ))
                .into_any_element(),
            self.list_card(
                "Clipboard Detail",
                &match state {
                    Some(state) => vec![
                        state
                            .latest_clipboard_event_id
                            .clone()
                            .map(|value| format!("Event: {value}"))
                            .unwrap_or_else(|| "Event: n/a".to_string()),
                        state
                            .latest_clipboard_source
                            .clone()
                            .map(|value| format!("Source: {value}"))
                            .unwrap_or_else(|| "Source: n/a".to_string()),
                        state.latest_clipboard_preview.clone(),
                    ],
                    None => Vec::new(),
                },
                "No clipboard record available.",
            )
            .into_any_element(),
        ]
    }
}
