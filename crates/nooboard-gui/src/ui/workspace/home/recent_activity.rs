use gpui::{
    AnimationExt as _, AnyElement, ClipboardItem, Context, Div, IntoElement, ParentElement, Styled,
    div, px,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{Icon, IconName, Sizable, StyledExt};

use crate::{
    ui::theme,
    workspace::recent_activity::{RecentActivitySeverity, RecentActivityViewState},
};

use super::{WorkspaceView, enter_animation};

impl WorkspaceView {
    fn recent_activity_copy_action(
        &self,
        item: &RecentActivityViewState,
        row_index: usize,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if matches!(item.severity, RecentActivitySeverity::Info) {
            return None;
        }

        let message = item.title.clone();
        Some(
            Button::new(format!("recent-activity-copy-{row_index}"))
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

    fn activity_icon(item: &RecentActivityViewState) -> IconName {
        match item.label {
            "Clipboard" => IconName::Copy,
            "Incoming Transfer" | "Transfer Complete" => IconName::Folder,
            "Network Starting" | "Network Running" | "Network Stopped" => IconName::Globe,
            _ => IconName::TriangleAlert,
        }
    }

    fn activity_accent(item: &RecentActivityViewState) -> gpui::Hsla {
        match item.severity {
            RecentActivitySeverity::Info => match item.label {
                "Clipboard" => theme::accent_blue(),
                "Incoming Transfer" | "Transfer Complete" => theme::accent_amber(),
                _ => theme::accent_cyan(),
            },
            RecentActivitySeverity::Warning => theme::accent_amber(),
            RecentActivitySeverity::Error => theme::accent_rose(),
        }
    }

    fn recent_activity_row(
        &self,
        item: &RecentActivityViewState,
        row_index: usize,
        cx: &mut Context<Self>,
    ) -> Div {
        let accent = Self::activity_accent(item);
        let copy_action = self.recent_activity_copy_action(item, row_index, cx);
        let label_row = {
            let row = div().h_flex().items_center().gap(px(8.0)).child(
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
                    .child(item.label),
            );

            match copy_action {
                Some(action) => row.child(action),
                None => row,
            }
        };

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
                        Icon::new(Self::activity_icon(item))
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
                            .child(label_row)
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

    pub(super) fn recent_activity_card(
        &self,
        activity: &[RecentActivityViewState],
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
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
