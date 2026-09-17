//! Platform capabilities, desktop preferences and durable window navigation intent.
pub mod lifecycle;
mod preferences;
pub mod tray;
use crate::errors::{UiError, ui};
pub use preferences::{Language, Patch};
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Manager, State};
use tokio::sync::watch;

pub const TRAY_SUPPORTED: bool = cfg!(any(target_os = "macos", target_os = "windows"));
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Page {
    Home,
    Transfers,
    Settings,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Navigation {
    pub id: u32,
    pub page: Page,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub revision: u32,
    pub tray_supported: bool,
    pub tray_available: bool,
    pub close_to_tray: bool,
    pub preference_error: bool,
    pub language: Language,
    pub resolved_language: String,
    pub visible: bool,
    pub navigation: Option<Navigation>,
    pub version: &'static str,
}
pub struct Desktop {
    preferences: Mutex<preferences::PreferenceStore>,
    pub state: watch::Sender<Snapshot>,
    pub exit: lifecycle::ExitControl,
}
impl Desktop {
    pub fn load(handle: &AppHandle) -> Self {
        let diagnostic = cfg!(all(
            debug_assertions,
            feature = "diagnostics",
            target_os = "macos"
        )) && std::env::var("NOOBOARD_DIAGNOSTIC").as_deref() == Ok("1");
        let path = if diagnostic {
            None
        } else {
            handle
                .path()
                .app_data_dir()
                .ok()
                .map(|p| p.join("desktop-preferences.json"))
        };
        let missing_path = !diagnostic && path.is_none();
        let mut preferences = preferences::PreferenceStore::load(path);
        preferences.failed |= missing_path;
        preferences.unavailable = missing_path;
        Self::from_preferences(preferences)
    }
    fn from_preferences(preferences: preferences::PreferenceStore) -> Self {
        let language = preferences.value.language.unwrap_or_default();
        let state = Snapshot {
            revision: 0,
            tray_supported: TRAY_SUPPORTED,
            tray_available: false,
            close_to_tray: preferences.value.close_to_tray && !preferences.failed,
            preference_error: preferences.failed,
            language,
            resolved_language: language.resolved().into(),
            visible: true,
            navigation: None,
            version: env!("CARGO_PKG_VERSION"),
        };
        Self {
            preferences: Mutex::new(preferences),
            state: watch::channel(state).0,
            exit: Default::default(),
        }
    }
    pub fn snapshot(&self) -> Snapshot {
        self.state.borrow().clone()
    }
    pub fn change(&self, update: impl FnOnce(&mut Snapshot)) {
        self.state.send_modify(|s| {
            update(s);
            s.revision = s.revision.wrapping_add(1);
        });
    }
    pub fn visible(&self, visible: bool) {
        if self.state.borrow().visible != visible {
            self.change(|s| s.visible = visible);
        }
    }
    fn acknowledge(&self, id: u32) {
        self.state.send_if_modified(|s| {
            if s.navigation.as_ref().is_some_and(|n| n.id == id) {
                s.navigation = None;
                s.revision = s.revision.wrapping_add(1);
                true
            } else {
                false
            }
        });
    }
    pub fn text(&self, key: &str) -> String {
        static EN: OnceLock<serde_json::Value> = OnceLock::new();
        static ZH: OnceLock<serde_json::Value> = OnceLock::new();
        let values = if self.state.borrow().resolved_language == "zh-CN" {
            ZH.get_or_init(|| {
                serde_json::from_str(include_str!("../../../src/i18n/locales/zh-CN/common.json"))
                    .unwrap()
            })
        } else {
            EN.get_or_init(|| {
                serde_json::from_str(include_str!("../../../src/i18n/locales/en/common.json"))
                    .unwrap()
            })
        };
        values[key].as_str().unwrap_or(key).into()
    }
}

#[tauri::command]
pub fn desktop_preferences(
    handle: AppHandle,
    desktop: State<'_, Desktop>,
    patch: Option<Patch>,
    legacy_language: Option<Language>,
) -> Result<Snapshot, UiError> {
    {
        let mut store = desktop
            .preferences
            .lock()
            .map_err(|_| ui("desktopPreferences"))?;
        store.update(patch.unwrap_or_default(), legacy_language, TRAY_SUPPORTED)?;
        let language = store.value.language.unwrap_or_default();
        desktop.change(|s| {
            s.close_to_tray = store.value.close_to_tray && !store.failed;
            s.preference_error = store.failed;
            s.language = language;
            s.resolved_language = language.resolved().into();
        });
    }
    tray::refresh(&handle);
    Ok(desktop.snapshot())
}

#[tauri::command]
pub fn desktop_navigation_ack(desktop: State<'_, Desktop>, id: u32) {
    desktop.acknowledge(id);
}

pub fn show(handle: &AppHandle, page: Option<Page>) {
    let Some(desktop) = handle.try_state::<Desktop>() else {
        return;
    };
    if desktop.exit.exiting() {
        return;
    }
    if let Some(page) = page {
        desktop.change(|s| {
            s.navigation = Some(Navigation {
                id: s.revision.wrapping_add(1),
                page,
            })
        });
    }
    let window = handle.get_webview_window("main").or_else(|| {
        let config = handle
            .config()
            .app
            .windows
            .iter()
            .find(|c| c.label == "main")?;
        tauri::WebviewWindowBuilder::from_config(handle, config)
            .ok()?
            .build()
            .ok()
    });
    if let Some(window) = window {
        let _ = window.unminimize();
        if window.show().is_ok() {
            desktop.visible(true);
            let _ = window.set_focus();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_navigation_ack_cannot_clear_a_newer_request() {
        let desktop = Desktop::from_preferences(preferences::PreferenceStore::load(None));
        desktop.change(|s| {
            s.navigation = Some(Navigation {
                id: 1,
                page: Page::Transfers,
            })
        });
        desktop.change(|s| {
            s.navigation = Some(Navigation {
                id: 2,
                page: Page::Settings,
            })
        });
        desktop.acknowledge(1);
        assert_eq!(desktop.snapshot().navigation.unwrap().id, 2);
        desktop.acknowledge(2);
        assert!(desktop.snapshot().navigation.is_none());
    }
    #[test]
    fn native_labels_share_complete_bilingual_frontend_resources() {
        let desktop = Desktop::from_preferences(preferences::PreferenceStore::load(None));
        for language in ["en", "zh-CN"] {
            desktop.change(|s| s.resolved_language = language.into());
            for key in [
                "starting",
                "startFailed",
                "trayOpen",
                "trayStatus",
                "trayPaused",
                "transfers",
                "settings",
                "menuQuit",
                "trayQuitTitle",
                "trayQuitMessage",
                "trayKeepRunning",
            ] {
                assert_ne!(desktop.text(key), key);
            }
        }
    }
}
