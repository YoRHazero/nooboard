use gpui::{AnyElement, Hsla, InteractiveElement, StatefulInteractiveElement};

use super::components::{clipboard_mode_tab, clipboard_themed_tooltip};
use super::page_state::ClipboardBroadcastScope;
use super::*;
use gpui_component::StyledExt;

impl WorkspaceView {
    pub(super) fn clipboard_targets_panel(
        &self,
        snapshot: &ClipboardSnapshot,
        cx: &mut Context<Self>,
    ) -> Div {
        let chips: Vec<_> = snapshot
            .target_rows
            .iter()
            .map(|target| self.clipboard_target_row(target, snapshot, cx))
            .collect();

        clipboard_panel_shell()
            .rounded(px(24.0))
            .v_flex()
            .gap(px(14.0))
            .p(px(20.0))
            .child(clipboard_panel_header(
                "Broadcast Targets",
                format!("{} connected peer(s)", snapshot.connected_target_count),
            ))
            .child(
                div()
                    .h_flex()
                    .gap(px(8.0))
                    .child(self.clipboard_scope_tab(
                        "All connected",
                        snapshot.broadcast_scope == ClipboardBroadcastScope::AllConnected,
                        theme::accent_blue(),
                        ClipboardBroadcastScope::AllConnected,
                        cx,
                    ))
                    .child(self.clipboard_scope_tab(
                        "Selected peers",
                        snapshot.broadcast_scope == ClipboardBroadcastScope::SelectedPeers,
                        theme::accent_cyan(),
                        ClipboardBroadcastScope::SelectedPeers,
                        cx,
                    )),
            )
            .child(
                div()
                    .w_full()
                    .overflow_x_scrollbar()
                    .child(div().h_flex().gap(px(10.0)).children(chips)),
            )
    }

    fn clipboard_scope_tab(
        &self,
        label: &'static str,
        selected: bool,
        accent: Hsla,
        scope: ClipboardBroadcastScope,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        clipboard_mode_tab(label, selected, accent)
            .id(format!("clipboard-scope-tab-{label}"))
            .cursor_pointer()
            .hover(|this| {
                this.bg(theme::bg_panel_alt())
                    .border_color(theme::border_strong())
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.set_clipboard_broadcast_scope(scope, cx);
            }))
            .into_any_element()
    }

    fn clipboard_target_row(
        &self,
        target: &snapshot::ClipboardTargetSnapshot,
        _snapshot: &ClipboardSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut chip = clipboard_target_chip(
            target.device_id.clone(),
            target.selected,
            target.interactive,
            theme::accent_cyan(),
        )
        .id(format!("clipboard-target-chip-{}", target.noob_id))
        .tooltip({
            let tooltip = format!("noob_id: {}", target.noob_id);
            move |window, cx| clipboard_themed_tooltip(tooltip.clone(), window, cx)
        });

        if target.interactive {
            let noob_id = target.noob_id.clone();
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
        }

        chip.into_any_element()
    }
}
