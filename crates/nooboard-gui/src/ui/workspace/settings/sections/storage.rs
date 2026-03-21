use gpui::{Context, Corner, IntoElement, ParentElement, Styled, Window, div, px};
use gpui_component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_component::menu::{DropdownMenu as _, PopupMenu, PopupMenuItem};
use gpui_component::input::Input;
use gpui_component::{IconName, Sizable, StyledExt};

use crate::{ui::theme, workspace::view_state::SettingsPageViewState};

use super::super::super::WorkspaceView;
use super::super::state::{
    SettingsSectionKey, StorageBytesUnit, StorageDurationUnit,
};

const STORAGE_FIELD_WIDTH: f32 = 336.0;
const STORAGE_UNIT_WIDTH: f32 = 104.0;

impl WorkspaceView {
    pub(in crate::ui::workspace::settings) fn storage_settings_panel(
        &self,
        state: &SettingsPageViewState,
        dirty: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let (status_label, status_accent) =
            self.settings_section_status(SettingsSectionKey::Storage, dirty);
        let actions_enabled = dirty && self.settings.applying().is_none();

        self.settings_section_shell(
            "Storage",
            "Control how long clipboard history is kept and how much text nooboard stores.",
            self.settings_status_chip(status_label, status_accent),
        )
        .child(self.settings_readonly_path_card(
            "Storage Folder",
            state.storage.db_root.display().to_string(),
        ))
        .child(
            div()
                .h_flex()
                .items_start()
                .flex_wrap()
                .gap(px(12.0))
                .child(self.settings_fixed_field_with_tooltip(
                    "settings-storage-history-window",
                    "History Window",
                    "How long saved clipboard history stays available.",
                    px(STORAGE_FIELD_WIDTH),
                    self.storage_measurement_surface(
                        Input::new(&self.settings.history_window_input())
                            .small()
                            .appearance(false)
                            .bordered(false)
                            .focus_bordered(false)
                            .w_full(),
                        self.storage_duration_unit_button(
                            "settings-history-window-unit",
                            self.settings.history_window_unit(),
                            Self::request_select_history_window_unit,
                            cx,
                        ),
                    ),
                ))
                .child(self.settings_fixed_field_with_tooltip(
                    "settings-storage-dedup-window",
                    "Dedup Window",
                    "How long identical clipboard items are treated as duplicates.",
                    px(STORAGE_FIELD_WIDTH),
                    self.storage_measurement_surface(
                        Input::new(&self.settings.dedup_window_input())
                            .small()
                            .appearance(false)
                            .bordered(false)
                            .focus_bordered(false)
                            .w_full(),
                        self.storage_duration_unit_button(
                            "settings-dedup-window-unit",
                            self.settings.dedup_window_unit(),
                            Self::request_select_dedup_window_unit,
                            cx,
                        ),
                    ),
                )),
        )
        .child(
            div()
                .h_flex()
                .items_start()
                .gap(px(12.0))
                .child(self.settings_fixed_field_with_tooltip(
                    "settings-storage-max-text-bytes",
                    "Max Text Bytes",
                    "The largest text item nooboard will save or edit.",
                    px(STORAGE_FIELD_WIDTH),
                    self.storage_measurement_surface(
                        Input::new(&self.settings.max_text_bytes_input())
                            .small()
                            .appearance(false)
                            .bordered(false)
                            .focus_bordered(false)
                            .w_full(),
                        self.storage_bytes_unit_button(
                            "settings-max-text-bytes-unit",
                            self.settings.max_text_bytes_unit(),
                            Self::request_select_max_text_bytes_unit,
                            cx,
                        ),
                    ),
                )),
        )
        .child(
            div()
                .h_flex()
                .justify_end()
                .gap(px(8.0))
                .child(self.settings_compact_action_button(
                    "settings-reset-storage",
                    "Reset",
                    "Undo the changes in this section."
                        .to_string(),
                    actions_enabled,
                    theme::accent_rose(),
                    |this, _, window, cx| {
                        this.request_reset_storage_settings(window, cx);
                    },
                    cx,
                ))
                .child(self.settings_compact_action_button(
                    "settings-apply-storage",
                    "Apply",
                    "Save the changes in this section.".to_string(),
                    actions_enabled,
                    theme::accent_cyan(),
                    |this, _, _, cx| {
                        this.request_apply_storage_settings(cx);
                    },
                    cx,
                )),
        )
    }

    fn storage_measurement_surface(
        &self,
        input: impl IntoElement,
        unit_button: impl IntoElement,
    ) -> impl IntoElement {
        div()
            .w_full()
            .h_flex()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .child(self.settings_input_surface(input)),
            )
            .child(div().w(px(STORAGE_UNIT_WIDTH)).flex_none().child(unit_button))
    }

    fn storage_duration_unit_button(
        &self,
        id_prefix: &str,
        selected: StorageDurationUnit,
        on_select: fn(
            &mut Self,
            StorageDurationUnit,
            &mut Window,
            &mut Context<Self>,
        ),
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let view = cx.entity().clone();

        self.storage_unit_dropdown_button(
            format!("{id_prefix}-button"),
            selected.label(),
            move |menu, window, _| {
                let mut menu: PopupMenu = menu;
                for unit in StorageDurationUnit::ALL {
                    let view = view.clone();
                    menu = menu.item(
                        PopupMenuItem::new(unit.label())
                            .checked(unit == selected)
                            .on_click(window.listener_for(&view, move |this, _, window, cx| {
                                on_select(this, unit, window, cx);
                            })),
                    );
                }
                menu
            },
            cx,
        )
    }

    fn storage_bytes_unit_button(
        &self,
        id_prefix: &str,
        selected: StorageBytesUnit,
        on_select: fn(
            &mut Self,
            StorageBytesUnit,
            &mut Window,
            &mut Context<Self>,
        ),
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let view = cx.entity().clone();

        self.storage_unit_dropdown_button(
            format!("{id_prefix}-button"),
            selected.label(),
            move |menu, window, _| {
                let mut menu: PopupMenu = menu;
                for unit in StorageBytesUnit::ALL {
                    let view = view.clone();
                    menu = menu.item(
                        PopupMenuItem::new(unit.label())
                            .checked(unit == selected)
                            .on_click(window.listener_for(&view, move |this, _, window, cx| {
                                on_select(this, unit, window, cx);
                            })),
                    );
                }
                menu
            },
            cx,
        )
    }

    fn storage_unit_dropdown_button(
        &self,
        id: impl Into<gpui::ElementId>,
        label: &str,
        menu: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let variant = ButtonCustomVariant::new(cx)
            .color(theme::bg_console())
            .foreground(theme::fg_primary())
            .hover(theme::bg_panel_alt())
            .active(theme::bg_panel())
            .shadow(false);

        Button::new(id)
            .custom(variant)
            .small()
            .compact()
            .w_full()
            .rounded(px(12.0))
            .border_1()
            .border_color(theme::border_soft())
            .child(
                div()
                    .w_full()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_semibold()
                            .text_color(theme::fg_primary())
                            .child(label.to_string()),
                    )
                    .child(
                        div()
                            .text_color(theme::fg_muted())
                            .child(IconName::ChevronDown),
                    ),
            )
            .dropdown_menu_with_anchor(Corner::BottomRight, menu)
    }
}
