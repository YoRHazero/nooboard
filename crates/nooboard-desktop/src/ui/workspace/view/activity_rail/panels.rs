use gpui::{Div, Hsla, ParentElement, Styled, div, px};
use gpui_component::{Icon, IconName, StyledExt};

use crate::ui::theme;

use super::super::{
    components::console_pill,
    shared::{activity_accent, activity_kind_icon},
};
use super::WorkspaceView;

impl WorkspaceView {
    pub(super) fn activity_section(
        &self,
        eyebrow: &str,
        title: &str,
        accent: Hsla,
        body: Div,
    ) -> Div {
        div()
            .v_flex()
            .gap(px(12.0))
            .p(px(16.0))
            .bg(theme::bg_rail_panel())
            .border_1()
            .border_color(theme::border_soft())
            .rounded(px(22.0))
            .shadow_xs()
            .child(
                div()
                    .v_flex()
                    .gap(px(10.0))
                    .child(
                        div()
                            .h(px(2.0))
                            .w_full()
                            .bg(accent.opacity(0.9))
                            .rounded(px(999.0)),
                    )
                    .child(
                        div()
                            .h_flex()
                            .justify_between()
                            .items_center()
                            .gap(px(10.0))
                            .child(
                                div()
                                    .h_flex()
                                    .gap(px(8.0))
                                    .items_center()
                                    .child(div().size(px(7.0)).rounded(px(999.0)).bg(accent))
                                    .child(
                                        div()
                                            .text_size(px(10.0))
                                            .font_semibold()
                                            .text_color(accent)
                                            .child(eyebrow.to_uppercase()),
                                    ),
                            )
                            .child(console_pill("rail", accent)),
                    )
                    .child(
                        div()
                            .text_size(px(16.0))
                            .font_semibold()
                            .text_color(theme::fg_primary())
                            .child(title.to_string()),
                    ),
            )
            .child(body)
    }

    pub(super) fn activity_panel(&self) -> Div {
        self.activity_section(
            "Signal Feed",
            "Live Feed",
            theme::accent_cyan(),
            div().v_flex().gap(px(12.0)).children(
                self.state.app.recent_activity.iter().take(3).map(|item| {
                    let accent = activity_accent(&item.kind);

                    div()
                        .h_flex()
                        .items_start()
                        .gap(px(12.0))
                        .p(px(12.0))
                        .bg(theme::bg_activity())
                        .border_1()
                        .border_color(theme::border_soft())
                        .rounded(px(16.0))
                        .child(
                            div()
                                .size(px(30.0))
                                .rounded(px(11.0))
                                .bg(accent.opacity(0.14))
                                .border_1()
                                .border_color(accent.opacity(0.28))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    Icon::new(activity_kind_icon(&item.kind))
                                        .size(px(14.0))
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
                                        .justify_between()
                                        .items_center()
                                        .gap(px(10.0))
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .font_semibold()
                                                .text_color(accent)
                                                .child(item.kind.clone()),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .text_color(theme::fg_muted())
                                                .child(item.time_label.clone()),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .text_color(theme::fg_primary())
                                        .line_clamp(2)
                                        .text_ellipsis()
                                        .child(item.title.clone()),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(theme::fg_muted())
                                        .line_clamp(2)
                                        .text_ellipsis()
                                        .child(item.detail.clone()),
                                ),
                        )
                }),
            ),
        )
    }

    pub(super) fn pending_files_panel(&self) -> Div {
        self.activity_section(
            "Inbound Queue",
            "Awaiting Review",
            theme::accent_amber(),
            div()
                .v_flex()
                .gap(px(12.0))
                .children(self.state.app.pending_files.iter().map(|item| {
                    div()
                        .h_flex()
                        .items_start()
                        .gap(px(10.0))
                        .p(px(12.0))
                        .bg(theme::bg_activity())
                        .border_1()
                        .border_color(theme::border_soft())
                        .rounded(px(16.0))
                        .child(
                            div()
                                .size(px(28.0))
                                .rounded(px(10.0))
                                .bg(theme::accent_amber().opacity(0.14))
                                .border_1()
                                .border_color(theme::accent_amber().opacity(0.28))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    Icon::new(IconName::Inbox)
                                        .size(px(14.0))
                                        .text_color(theme::accent_amber()),
                                ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.0))
                                .v_flex()
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .font_semibold()
                                        .text_color(theme::fg_primary())
                                        .line_clamp(2)
                                        .text_ellipsis()
                                        .child(item.file_name.clone()),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(theme::fg_muted())
                                        .truncate()
                                        .child(format!(
                                            "{} · {}",
                                            item.peer_label, item.size_label
                                        )),
                                ),
                        )
                })),
        )
    }

    pub(super) fn error_panel(&self) -> Div {
        self.activity_section(
            "Operator Note",
            "System Notes",
            theme::accent_rose(),
            div()
                .v_flex()
                .gap(px(10.0))
                .child(
                    div()
                        .p(px(12.0))
                        .bg(theme::bg_activity())
                        .border_1()
                        .border_color(theme::border_soft())
                        .rounded(px(16.0))
                        .text_size(px(12.0))
                        .text_color(theme::fg_secondary())
                        .line_clamp(3)
                        .text_ellipsis()
                        .child("The desktop shell is still running on mocked runtime data, but the current layout is already tuned for a denser control-room feel and cleaner information grouping."),
                ),
        )
    }
}
