use gpui::{AnimationExt as _, Div, IntoElement, ParentElement, Styled};

use crate::state::ActivityItem;

use super::super::{
    WorkspaceView,
    shared::{activity_accent, activity_kind_icon, enter_animation},
};
use super::components::{
    recent_activity_card_header, recent_activity_card_shell, recent_activity_row,
};

impl WorkspaceView {
    fn recent_activity_row(item: &ActivityItem) -> Div {
        let accent = activity_accent(&item.kind);

        recent_activity_row(
            item.kind.clone(),
            item.time_label.clone(),
            item.title.clone(),
            activity_kind_icon(&item.kind),
            accent,
        )
    }

    pub(super) fn recent_activity_card(&self) -> impl IntoElement {
        let activity: Vec<_> = self
            .state
            .app
            .recent_activity
            .iter()
            .rev()
            .take(5)
            .collect();

        recent_activity_card_shell()
            .child(recent_activity_card_header(activity.len()))
            .children(activity.into_iter().map(Self::recent_activity_row))
            .with_animation("recent-activity-card", enter_animation(), |this, delta| {
                this.opacity(0.35 + delta * 0.65)
            })
    }
}
