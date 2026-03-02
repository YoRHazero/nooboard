use gpui::{
    AnimationExt as _, Div, Hsla, IntoElement, ParentElement, Styled, div,
    prelude::FluentBuilder as _, px,
};
use gpui_component::progress::Progress;
use gpui_component::{Icon, IconName, StyledExt};

use crate::state::{PendingFileDecision, TransferItem};
use crate::ui::theme;

use super::super::{
    WorkspaceView,
    components::{command_bar, data_cell, section_header},
    shared::{activity_accent, activity_kind_icon, enter_animation},
};

impl WorkspaceView {
    fn intake_posture(&self) -> (&'static str, String, Hsla) {
        let count = self.state.app.pending_files.len();

        if count == 0 {
            (
                "CLEAR",
                "No inbound files are waiting for operator approval.".into(),
                theme::accent_green(),
            )
        } else if count <= 2 {
            (
                "WATCH",
                "Inbound files are staged for review without creating intake pressure.".into(),
                theme::accent_amber(),
            )
        } else {
            (
                "STACKED",
                "The intake shelf is accumulating and would benefit from a dedicated pass.".into(),
                theme::accent_rose(),
            )
        }
    }

    fn transfer_pacing(&self) -> (&'static str, String, Hsla) {
        if self.state.app.transfers.is_empty() {
            return (
                "IDLE",
                "No transfer lanes are moving, so the queue is waiting on the next dispatch."
                    .into(),
                theme::accent_blue(),
            );
        }

        let average_progress = self
            .state
            .app
            .transfers
            .iter()
            .map(|item| item.progress)
            .sum::<f32>()
            / self.state.app.transfers.len() as f32;

        let lead_speed = self
            .state
            .app
            .transfers
            .first()
            .map(|item| item.speed_label.clone())
            .unwrap_or_else(|| "speed unknown".into());

        if average_progress >= 0.55 {
            (
                "ADVANCING",
                format!(
                    "The lead lane is reporting {} and the active queue is moving cleanly.",
                    lead_speed
                ),
                theme::accent_green(),
            )
        } else {
            (
                "SPIN-UP",
                format!(
                    "The transfer surface is still ramping and the lead lane is reporting {}.",
                    lead_speed
                ),
                theme::accent_blue(),
            )
        }
    }

    fn routing_posture(&self) -> (&'static str, String, Hsla) {
        if self.state.app.manual_peers > 0 {
            (
                "PINNED",
                "Manual bootstrap peers are supplementing discovery so the route map stays anchored during churn.".into(),
                theme::accent_cyan(),
            )
        } else {
            (
                "AUTO",
                "Discovery is operating without pinned peers and the mesh is relying on broadcast visibility alone.".into(),
                theme::accent_green(),
            )
        }
    }

    fn activity_timeline_row(
        &self,
        index: usize,
        total: usize,
        time: &str,
        kind: &str,
        title: &str,
        detail: &str,
    ) -> Div {
        let accent = activity_accent(kind);
        let is_last = index + 1 == total;

        div()
            .h_flex()
            .items_start()
            .gap(px(16.0))
            .child(
                div()
                    .v_flex()
                    .items_center()
                    .pt(px(6.0))
                    .child(div().size(px(10.0)).rounded(px(999.0)).bg(accent))
                    .when(!is_last, |this| {
                        this.child(
                            div()
                                .mt(px(6.0))
                                .w(px(1.0))
                                .h(px(60.0))
                                .bg(theme::border_base()),
                        )
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .v_flex()
                    .gap(px(10.0))
                    .p(px(16.0))
                    .bg(theme::bg_console())
                    .border_1()
                    .border_color(theme::border_soft())
                    .rounded(px(20.0))
                    .child(
                        div()
                            .h_flex()
                            .justify_between()
                            .items_start()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .h_flex()
                                    .items_center()
                                    .gap(px(10.0))
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
                                                Icon::new(activity_kind_icon(kind))
                                                    .size(px(14.0))
                                                    .text_color(accent),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .px(px(10.0))
                                            .py(px(6.0))
                                            .bg(accent.opacity(0.14))
                                            .border_1()
                                            .border_color(accent.opacity(0.28))
                                            .rounded(px(999.0))
                                            .text_size(px(10.0))
                                            .font_semibold()
                                            .text_color(accent)
                                            .child(kind.to_string()),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(theme::fg_muted())
                                    .child(time.to_string()),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(15.0))
                            .font_semibold()
                            .text_color(theme::fg_primary())
                            .line_clamp(2)
                            .text_ellipsis()
                            .child(title.to_string()),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme::fg_secondary())
                            .line_clamp(3)
                            .text_ellipsis()
                            .child(detail.to_string()),
                    ),
            )
    }

    pub(super) fn recent_activity_card(&self) -> impl IntoElement {
        let app = &self.state.app;

        div()
            .v_flex()
            .gap(px(18.0))
            .p(px(22.0))
            .bg(theme::bg_panel())
            .border_1()
            .border_color(theme::border_base())
            .rounded(px(24.0))
            .shadow_xs()
            .child(command_bar(
                "signal ledger",
                "temporal trace of clipboard, transfer, and anomaly events",
                theme::accent_cyan(),
            ))
            .child(section_header(
                "Signal Trace",
                "Recent Activity",
                "Recent replicated events are arranged as an operator timeline so the sequence is readable at a glance.",
                theme::accent_cyan(),
            ))
            .children(app.recent_activity.iter().enumerate().map(|(index, item)| {
                self.activity_timeline_row(
                    index,
                    app.recent_activity.len(),
                    &item.time_label,
                    &item.kind,
                    &item.title,
                    &item.detail,
                )
            }))
            .with_animation("recent-activity-card", enter_animation(), |this, delta| {
                this.opacity(0.35 + delta * 0.65)
            })
    }

    fn transfer_lane(item: &TransferItem) -> Div {
        let accent = if item.progress >= 0.5 {
            theme::accent_green()
        } else {
            theme::accent_blue()
        };

        div()
            .v_flex()
            .gap(px(12.0))
            .p(px(16.0))
            .bg(theme::bg_panel_alt())
            .border_1()
            .border_color(theme::border_base())
            .rounded(px(20.0))
            .child(div().h(px(3.0)).w_full().bg(accent).rounded(px(999.0)))
            .child(
                div()
                    .h_flex()
                    .justify_between()
                    .items_start()
                    .gap(px(12.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .text_size(px(15.0))
                            .font_semibold()
                            .text_color(theme::fg_primary())
                            .truncate()
                            .child(item.file_name.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_semibold()
                            .text_color(accent)
                            .child(format!("{:.0}%", item.progress * 100.0)),
                    ),
            )
            .child(Progress::new().value(item.progress))
            .child(
                div()
                    .h_flex()
                    .justify_between()
                    .gap(px(12.0))
                    .text_size(px(12.0))
                    .text_color(theme::fg_muted())
                    .child(item.bytes_label.clone())
                    .child(item.speed_label.clone())
                    .child(item.eta_label.clone()),
            )
    }

    fn pending_review_row(item: &PendingFileDecision) -> Div {
        div()
            .h_flex()
            .items_start()
            .gap(px(12.0))
            .p(px(14.0))
            .bg(theme::bg_panel_alt())
            .border_1()
            .border_color(theme::border_base())
            .rounded(px(18.0))
            .child(
                div()
                    .mt(px(2.0))
                    .size(px(26.0))
                    .rounded(px(10.0))
                    .bg(theme::accent_amber().opacity(0.14))
                    .border_1()
                    .border_color(theme::accent_amber().opacity(0.3))
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
                            .text_size(px(13.0))
                            .font_semibold()
                            .text_color(theme::fg_primary())
                            .truncate()
                            .child(item.file_name.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme::fg_muted())
                            .truncate()
                            .child(format!("{} · {}", item.peer_label, item.size_label)),
                    ),
            )
    }

    pub(super) fn transfer_queue_card(&self) -> impl IntoElement {
        let app = &self.state.app;
        let (flow_label, flow_detail, flow_accent) = self.transfer_pacing();
        let (intake_label, intake_detail, intake_accent) = self.intake_posture();
        let (routing_label, routing_detail, routing_accent) = self.routing_posture();

        div()
            .v_flex()
            .gap(px(18.0))
            .p(px(22.0))
            .bg(theme::bg_panel())
            .border_1()
            .border_color(theme::border_base())
            .rounded(px(24.0))
            .shadow_xs()
            .child(command_bar(
                "lane control",
                "transfer pacing, intake posture, and mesh routing guidance",
                theme::accent_amber(),
            ))
            .child(section_header(
                "Lane Monitor",
                "Transfer Queue",
                "Active lanes and inbox reviews are grouped here so the home surface keeps operational pressure in one place.",
                theme::accent_amber(),
            ))
            .child(
                div()
                    .grid()
                    .grid_cols(3)
                    .gap(px(12.0))
                    .child(data_cell(
                        "Flow Pacing",
                        flow_label,
                        flow_detail,
                        flow_accent,
                    ))
                    .child(data_cell(
                        "Intake Posture",
                        intake_label,
                        intake_detail,
                        intake_accent,
                    ))
                    .child(data_cell(
                        "Routing Mode",
                        routing_label,
                        routing_detail,
                        routing_accent,
                    )),
            )
            .child(
                div()
                    .v_flex()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_semibold()
                            .text_color(theme::fg_secondary())
                            .child("ACTIVE LANES"),
                    )
                    .children(app.transfers.iter().map(Self::transfer_lane)),
            )
            .child(
                div()
                    .v_flex()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_semibold()
                            .text_color(theme::fg_secondary())
                            .child("AWAITING REVIEW"),
                    )
                    .children(app.pending_files.iter().map(Self::pending_review_row)),
            )
            .with_animation("transfer-queue-card", enter_animation(), |this, delta| {
                this.opacity(0.35 + delta * 0.65)
            })
    }
}
