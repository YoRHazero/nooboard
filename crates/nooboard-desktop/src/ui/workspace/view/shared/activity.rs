use gpui::Hsla;
use gpui_component::IconName;

use crate::ui::theme;

pub(crate) fn activity_kind_icon(kind: &str) -> IconName {
    if kind.contains("Error") {
        IconName::Bell
    } else if kind.contains("Transfer") {
        IconName::Folder
    } else {
        IconName::Copy
    }
}

pub(crate) fn activity_accent(kind: &str) -> Hsla {
    if kind.contains("Error") {
        theme::accent_rose()
    } else if kind.contains("Transfer") {
        theme::accent_amber()
    } else {
        theme::accent_cyan()
    }
}
