use gpui::{AnimationExt as _, Div, IntoElement, ParentElement, Styled};

use crate::state::live_app::RecentActivityItem;

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
    fn recent_activity_row(item: &RecentActivityItem) -> Div {
        let accent = activity_accent(item);

        recent_activity_row(
            activity_kind_label(item).to_string(),
            activity_time_label(item),
            activity_title(item),
            activity_kind_icon(item),
            accent,
        )
    }

    pub(super) fn recent_activity_card(&self, activity: &[RecentActivityItem]) -> impl IntoElement {
        recent_activity_card_shell()
            .child(recent_activity_card_header(activity.len()))
            .children(activity.iter().map(Self::recent_activity_row))
            .with_animation("recent-activity-card", enter_animation(), |this, delta| {
                this.opacity(0.35 + delta * 0.65)
            })
    }
}
