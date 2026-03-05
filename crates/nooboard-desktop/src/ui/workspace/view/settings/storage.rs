use gpui::{
    Context, Div, InteractiveElement, ParentElement, StatefulInteractiveElement, Styled, div, px,
};
use gpui_component::StyledExt;

use crate::ui::theme;

use super::WorkspaceView;

impl WorkspaceView {
    pub(super) fn storage_settings_panel(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex_1()
            .min_w(px(0.0))
            .v_flex()
            .gap(px(14.0))
            .p(px(18.0))
            .bg(theme::bg_panel())
            .border_1()
            .border_color(theme::border_base())
            .rounded(px(22.0))
            .shadow_xs()
            .child(
                div()
                    .text_size(px(18.0))
                    .font_semibold()
                    .text_color(theme::fg_primary())
                    .child("Storage"),
            )
            .child(div().h(px(1.0)).w_full().bg(theme::border_soft()))
            .child(self.settings_db_root_row(cx))
            .child(self.settings_field_row(
                "retain_versions",
                self.settings_page_state.storage_retain_versions.clone(),
            ))
            .child(self.settings_field_row(
                "history_days",
                self.settings_page_state.storage_history_days.clone(),
            ))
            .child(self.settings_field_row(
                "dedup_days",
                self.settings_page_state.storage_dedup_days.clone(),
            ))
            .child(self.settings_field_row(
                "gc_every_inserts",
                self.settings_page_state.storage_gc_every_inserts.clone(),
            ))
            .child(self.settings_field_row(
                "gc_batch_size",
                self.settings_page_state.storage_gc_batch_size.clone(),
            ))
            .child(
                div()
                    .pt(px(8.0))
                    .child(
                        self.settings_action_button(
                            "settings-save-storage-patch",
                            "Save Storage Patch",
                            theme::accent_cyan(),
                            cx,
                        )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.save_storage_patch(cx);
                            })),
                    ),
            )
    }

    fn settings_db_root_row(&self, cx: &mut Context<Self>) -> Div {
        let folder_label = self.settings_page_state.storage_db_root.display().to_string();

        div()
            .v_flex()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::fg_secondary())
                    .child("db_root"),
            )
            .child(
                div()
                    .id("settings-storage-db-root")
                    .w_full()
                    .min_w(px(0.0))
                    .px(px(12.0))
                    .py(px(10.0))
                    .bg(theme::bg_console())
                    .border_1()
                    .border_color(theme::border_soft())
                    .rounded(px(14.0))
                    .cursor_pointer()
                    .hover(|this| {
                        this.bg(theme::bg_panel_alt())
                            .border_color(theme::border_strong())
                    })
                    .active(|this| this.bg(theme::bg_panel()))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.pick_settings_db_root(window, cx);
                    }))
                    .child(
                        div()
                            .w_full()
                            .min_w(px(0.0))
                            .text_size(px(12.0))
                            .text_color(theme::fg_secondary())
                            .line_clamp(1)
                            .text_ellipsis()
                            .child(folder_label),
                    ),
            )
    }
}
