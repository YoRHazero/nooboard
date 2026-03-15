use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px};
use gpui_component::StyledExt;

use crate::ui::theme;

use super::{WorkspaceRenderModel, WorkspaceView};

impl WorkspaceView {
    pub(super) fn home_page(&self, model: &WorkspaceRenderModel) -> Vec<AnyElement> {
        let mut sections = Vec::new();

        if let Some(state) = model.page.as_ref() {
            sections.push(
                div()
                    .h_flex()
                    .flex_wrap()
                    .gap(px(12.0))
                    .children(
                        state
                            .home
                            .metrics
                            .iter()
                            .map(|metric| self.metric_card(metric)),
                    )
                    .into_any_element(),
            );
            sections.push(
                self.list_card(
                    "Latest Clipboard",
                    &[
                        state.home.latest_clipboard_preview.clone(),
                        state
                            .home
                            .latest_clipboard_event_id
                            .clone()
                            .map(|value| format!("Event: {value}"))
                            .unwrap_or_else(|| "Event: n/a".to_string()),
                        state
                            .home
                            .latest_clipboard_source
                            .clone()
                            .map(|value| format!("Source: {value}"))
                            .unwrap_or_else(|| "Source: n/a".to_string()),
                    ],
                    "No clipboard data.",
                )
                .into_any_element(),
            );
        }

        sections.push(
            div()
                .v_flex()
                .gap(px(10.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_semibold()
                        .text_color(theme::fg_secondary())
                        .child("RECENT ACTIVITY"),
                )
                .children(
                    model
                        .shell
                        .recent_activity
                        .iter()
                        .enumerate()
                        .map(|(index, item)| self.activity_row(item, index)),
                )
                .into_any_element(),
        );

        sections
    }
}
