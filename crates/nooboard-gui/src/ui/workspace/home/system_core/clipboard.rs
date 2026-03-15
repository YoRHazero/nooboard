use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    div, px,
};
use gpui_component::{Icon, IconName};

use crate::{
    ui::theme,
    workspace::{
        actions::clipboard as clipboard_actions,
        view_state::{HomeClipboardRecordViewState, HomeSystemCoreViewState},
    },
};

use super::{
    CLIPBOARD_PANEL_HEIGHT, CLIPBOARD_PANEL_WIDTH, WorkspaceView,
    components::{clipboard_action_placeholder, clipboard_action_shell, clipboard_read_board},
};

impl WorkspaceView {
    fn clipboard_adopt_action(
        &self,
        event_id: nooboard_core::EventId,
        accent: gpui::Hsla,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        clipboard_action_shell(accent)
            .id("system-core-clipboard-adopt-shell")
            .hover(|this| {
                this.bg(theme::bg_panel_highlight())
                    .border_color(accent.opacity(0.3))
            })
            .active(|this| {
                this.bg(theme::bg_panel())
                    .border_color(accent.opacity(0.24))
            })
            .tooltip(move |window, cx| {
                Self::themed_tooltip(
                    "Write committed text to the local clipboard".into(),
                    window,
                    cx,
                )
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                if let Some(task) =
                    clipboard_actions::adopt_clipboard_record_task(&this.controller, event_id, cx)
                {
                    task.detach();
                }
            }))
            .child(Icon::new(IconName::Copy).size(px(15.0)).text_color(accent))
    }

    fn clipboard_read_panel(
        &self,
        item: &HomeClipboardRecordViewState,
        accent: gpui::Hsla,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        clipboard_read_board(
            item.device_label.clone(),
            item.recorded_at_label.clone(),
            accent,
            self.clipboard_adopt_action(item.event_id, accent, cx)
                .into_any_element(),
            item.content.clone(),
        )
    }

    pub(super) fn clipboard_panel(
        &self,
        snapshot: &HomeSystemCoreViewState,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let content = match &snapshot.clipboard.latest_record {
            Some(record) => {
                let accent = if matches!(
                    record.source,
                    nooboard_core::ClipboardRecordSource::RemoteSync
                ) {
                    theme::accent_blue()
                } else {
                    theme::accent_green()
                };
                self.clipboard_read_panel(record, accent, cx)
                    .into_any_element()
            }
            None => clipboard_read_board(
                "No committed record".to_string(),
                "waiting for clipboard commit".to_string(),
                theme::border_soft(),
                clipboard_action_placeholder(theme::border_soft()).into_any_element(),
                "The Home panel shows the latest committed clipboard record available from \
                 nooboard-core."
                    .to_string(),
            )
            .into_any_element(),
        };

        div()
            .w(px(CLIPBOARD_PANEL_WIDTH))
            .flex_shrink_0()
            .h(px(CLIPBOARD_PANEL_HEIGHT))
            .bg(theme::bg_console())
            .border_1()
            .border_color(theme::border_soft())
            .rounded(px(24.0))
            .shadow_xs()
            .child(content)
    }
}
