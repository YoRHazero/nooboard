use gpui::{AnimationExt as _, Div, IntoElement, ParentElement, Styled, div, px};
use gpui_component::{Icon, StyledExt};

use crate::state::ActivityItem;
use crate::ui::theme;

use super::super::{
    WorkspaceView,
    shared::{activity_accent, activity_kind_icon, enter_animation},
};

impl WorkspaceView {
    fn recent_activity_row(item: &ActivityItem) -> Div {
        let accent = activity_accent(&item.kind);

        div()
            .h_flex()
            .items_start()
            .gap(px(14.0))
            .p(px(16.0))
            .bg(theme::bg_panel_alt())
            .border_1()
            .border_color(theme::border_soft())
            .rounded(px(20.0))
            .child(
                div()
                    .mt(px(2.0))
                    .size(px(34.0))
                    .rounded(px(12.0))
                    .bg(accent.opacity(0.14))
                    .border_1()
                    .border_color(accent.opacity(0.28))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(activity_kind_icon(&item.kind))
                            .size(px(16.0))
                            .text_color(accent),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .v_flex()
                    .gap(px(8.0))
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .justify_between()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .px(px(10.0))
                                    .py(px(5.0))
                                    .rounded(px(999.0))
                                    .bg(accent.opacity(0.14))
                                    .border_1()
                                    .border_color(accent.opacity(0.28))
                                    .text_size(px(10.0))
                                    .font_semibold()
                                    .text_color(accent)
                                    .child(item.kind.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(theme::fg_muted())
                                    .child(item.time_label.clone()),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_semibold()
                            .text_color(theme::fg_primary())
                            .line_clamp(2)
                            .text_ellipsis()
                            .child(item.title.clone()),
                    ),
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

        div()
            .v_flex()
            .gap(px(18.0))
            .p(px(22.0))
            .bg(theme::bg_panel())
            .border_1()
            .border_color(theme::border_base())
            .rounded(px(24.0))
            .shadow_xs()
            .child(
                div()
                    .h_flex()
                    .items_end()
                    .justify_between()
                    .gap(px(16.0))
                    .child(
                        div()
                            .v_flex()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_semibold()
                                    .text_color(theme::accent_cyan())
                                    .child("RECENT ACTIVITY"),
                            )
                            .child(
                                div()
                                    .text_size(px(24.0))
                                    .font_semibold()
                                    .text_color(theme::fg_primary())
                                    .child("Recent Activity"),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme::fg_muted())
                            .child(format!("{} items", activity.len())),
                    ),
            )
            .children(activity.into_iter().map(Self::recent_activity_row))
            .with_animation("recent-activity-card", enter_animation(), |this, delta| {
                this.opacity(0.35 + delta * 0.65)
            })
    }
}
