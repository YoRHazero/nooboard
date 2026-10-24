use crate::{app::snapshot::Snapshot, ipc::contracts};
pub fn snapshot(value: Snapshot) -> contracts::Snapshot {
    contracts::Snapshot {
        revision: value.revision,
        tray_supported: value.tray.supported,
        tray_available: value.tray.available,
        close_to_tray: value.preferences.close_to_tray,
        preference_error: value.preferences.failed,
        language: value.preferences.language,
        resolved_language: value.preferences.resolved_language,
        visible: value.window.visible,
        navigation: value.window.navigation,
        version: env!("CARGO_PKG_VERSION"),
    }
}
