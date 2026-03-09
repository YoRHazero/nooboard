use super::super::snapshot::{HomeClipboardRecordSnapshot, HomeSystemCoreSnapshot};
use super::*;
use crate::state::live_commands;
use gpui_component::{Icon, IconName};

impl WorkspaceView {
    fn clipboard_adopt_action(
        &self,
        event_id: nooboard_app::EventId,
        accent: Hsla,
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
            .tooltip(move |window: &mut Window, cx| {
                Self::themed_tooltip(
                    "Write committed text to the local clipboard".into(),
                    window,
                    cx,
                )
            })
            .on_click(cx.listener(move |_, _, _, cx| {
                live_commands::adopt_clipboard_record(event_id, cx);
            }))
            .child(Icon::new(IconName::Copy).size(px(15.0)).text_color(accent))
    }

    fn clipboard_read_board(
        &self,
        item: &HomeClipboardRecordSnapshot,
        accent: Hsla,
        adopt_event_id: Option<nooboard_app::EventId>,
        cx: &mut Context<Self>,
    ) -> Div {
        clipboard_read_board(
            item.device_label.clone(),
            item.recorded_at_label.clone(),
            accent,
            if let Some(event_id) = adopt_event_id {
                self.clipboard_adopt_action(event_id, accent, cx)
                    .into_any_element()
            } else {
                clipboard_action_placeholder(accent).into_any_element()
            },
            item.content.clone(),
        )
    }

    pub(super) fn clipboard_panel(
        &self,
        snapshot: &HomeSystemCoreSnapshot,
        cx: &mut Context<Self>,
    ) -> Div {
        let content = match &snapshot.clipboard.latest_record {
            Some(record) => {
                let accent = if matches!(record.source, nooboard_app::ClipboardRecordSource::RemoteSync)
                {
                    theme::accent_blue()
                } else {
                    theme::accent_green()
                };
                self.clipboard_read_board(record, accent, snapshot.clipboard.adopt_event_id, cx)
            }
            None => clipboard_read_board(
                "No committed record".to_string(),
                "waiting for clipboard commit".to_string(),
                theme::border_soft(),
                clipboard_action_placeholder(theme::border_soft()).into_any_element(),
                "The Home panel only shows the latest committed clipboard record from the app service."
                    .to_string(),
            ),
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
