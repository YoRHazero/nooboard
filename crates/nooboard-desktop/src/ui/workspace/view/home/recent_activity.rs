use gpui::{AnimationExt as _, ClipboardItem, Context, Div, IntoElement, ParentElement, Styled};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{IconName, Sizable};

use crate::state::live_app::{RecentActivityItem, RecentActivitySeverity};

use super::super::{
    WorkspaceView,
    shared::{
        activity_accent, activity_kind_icon, activity_kind_label, activity_time_label,
        activity_title, enter_animation,
    },
};
use super::components::{
    recent_activity_card_header, recent_activity_card_shell, recent_activity_row,
};

impl WorkspaceView {
    fn recent_activity_row(
        &self,
        item: &RecentActivityItem,
        row_index: usize,
        cx: &mut Context<Self>,
    ) -> Div {
        let accent = activity_accent(item);
        let title = activity_title(item);
        let copy_action = self.recent_activity_copy_action(item, row_index, title.clone(), cx);

        recent_activity_row(
            activity_kind_label(item).to_string(),
            activity_time_label(item),
            title,
            activity_kind_icon(item),
            accent,
            copy_action,
        )
    }

    fn recent_activity_copy_action(
        &self,
        item: &RecentActivityItem,
        row_index: usize,
        message: String,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if matches!(item.severity, RecentActivitySeverity::Info) {
            return None;
        }

        let button_id = format!("recent-activity-copy-{row_index}");
        Some(
            Button::new(button_id)
                .ghost()
                .xsmall()
                .icon(IconName::Copy)
                .tooltip("Copy activity message")
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.copy_recent_activity_message(message.clone(), cx);
                }))
                .into_any_element(),
        )
    }

    fn copy_recent_activity_message(&mut self, message: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(message));
        cx.notify();
    }

    pub(super) fn recent_activity_card(
        &self,
        activity: &[RecentActivityItem],
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        recent_activity_card_shell()
            .child(recent_activity_card_header(activity.len()))
            .children(
                activity
                    .iter()
                    .enumerate()
                    .map(|(row_index, item)| self.recent_activity_row(item, row_index, cx)),
            )
            .with_animation("recent-activity-card", enter_animation(), |this, delta| {
                this.opacity(0.35 + delta * 0.65)
            })
    }
}
