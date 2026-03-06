use super::*;

impl WorkspaceView {
    pub(super) fn clipboard_targets_panel(&self, cx: &mut Context<Self>) -> Div {
        let chips: Vec<_> = self
            .state
            .app
            .clipboard
            .targets
            .iter()
            .map(|target| self.clipboard_target_chip(target, cx))
            .collect();

        div()
            .v_flex()
            .gap(px(14.0))
            .p(px(20.0))
            .bg(theme::bg_panel())
            .border_1()
            .border_color(theme::border_base())
            .rounded(px(24.0))
            .shadow_xs()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(16.0))
                    .child(
                        div()
                            .text_size(px(16.0))
                            .font_semibold()
                            .text_color(theme::fg_primary())
                            .child("Targets"),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme::fg_muted())
                            .child(format!(
                                "{} of {} selected",
                                self.clipboard_page.selected_target_count(),
                                self.state.app.clipboard.targets.len()
                            )),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .overflow_x_scrollbar()
                    .child(div().h_flex().gap(px(10.0)).children(chips)),
            )
    }

    fn clipboard_target_chip(
        &self,
        target: &ClipboardTarget,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let accent = if target.status == ClipboardTargetStatus::Connected {
            theme::accent_cyan()
        } else {
            theme::fg_muted()
        };
        let selected = self.clipboard_page.target_is_selected(&target.noob_id);
        let noob_id = target.noob_id.clone();
        let tooltip = format!("noob_id: {}", target.noob_id);
        let mut chip = div()
            .id(format!("clipboard-target-chip-{}", target.noob_id))
            .min_w(px(152.0))
            .px(px(14.0))
            .py(px(12.0))
            .rounded(px(18.0))
            .bg(if selected {
                theme::bg_panel_highlight()
            } else {
                theme::bg_console()
            })
            .border_1()
            .border_color(if selected {
                accent.opacity(0.34)
            } else {
                theme::border_soft()
            })
            .tooltip(move |window: &mut Window, cx| {
                Self::clipboard_themed_tooltip(tooltip.clone(), window, cx)
            })
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_semibold()
                            .text_color(if target.is_connected() {
                                theme::fg_primary()
                            } else {
                                theme::fg_secondary()
                            })
                            .child(target.device_id.clone()),
                    )
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(div().size(px(6.0)).rounded(px(999.0)).bg(
                                if target.is_connected() {
                                    accent
                                } else {
                                    theme::border_base()
                                },
                            ))
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .font_semibold()
                                    .text_color(if target.is_connected() {
                                        accent
                                    } else {
                                        theme::fg_muted()
                                    })
                                    .child(if target.is_connected() {
                                        "Connected"
                                    } else {
                                        "Offline"
                                    }),
                            ),
                    ),
            );

        if target.is_connected() {
            chip = chip
                .cursor_pointer()
                .hover(|this| {
                    this.bg(theme::bg_panel_alt())
                        .border_color(theme::border_strong())
                })
                .active(|this| this.bg(theme::bg_panel()))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.toggle_clipboard_target(&noob_id, cx);
                }));
        } else {
            chip = chip.opacity(0.72);
        }

        chip.into_any_element()
    }
}
