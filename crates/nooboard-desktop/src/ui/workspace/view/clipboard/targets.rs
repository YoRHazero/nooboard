use super::*;
use gpui::StatefulInteractiveElement;
use crate::ui::workspace::view::clipboard::components::clipboard_themed_tooltip;

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

        clipboard_panel_shell()
            .rounded(px(24.0))
            .v_flex()
            .gap(px(14.0))
            .p(px(20.0))
            .child(clipboard_panel_header(
                "Targets",
                format!(
                    "{} of {} selected",
                    self.clipboard_page.selected_target_count(),
                    self.state.app.clipboard.targets.len()
                ),
            ))
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
        let mut chip = clipboard_target_chip(
            target.device_id.clone(),
            target.is_connected(),
            selected,
            accent,
        )
            .id(format!("clipboard-target-chip-{}", target.noob_id))
            .tooltip(move |window: &mut Window, cx| {
                clipboard_themed_tooltip(tooltip.clone(), window, cx)
            });

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
