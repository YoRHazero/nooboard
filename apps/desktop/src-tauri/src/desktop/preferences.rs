//! Desktop-only preferences; business settings continue to live in core.
use crate::errors::{UiError, ui};
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

/// Windows can report NotFound when an ancestor is a file, unlike Unix's NotADirectory.
fn missing_file_is_valid(path: &Path) -> bool {
    for parent in path.ancestors().skip(1) {
        match std::fs::metadata(parent) {
            Ok(metadata) => return metadata.is_dir(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return false,
        }
    }
    true
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub enum Language {
    #[default]
    #[serde(rename = "system")]
    System,
    #[serde(rename = "en")]
    English,
    #[serde(rename = "zh-CN")]
    Chinese,
}
impl Language {
    pub fn resolved(self) -> &'static str {
        match self {
            Self::Chinese => "zh-CN",
            Self::English => "en",
            Self::System => {
                let locale = tauri_plugin_os::locale().unwrap_or_default().to_lowercase();
                if locale == "zh" || locale.starts_with("zh-") || locale.starts_with("zh_") {
                    "zh-CN"
                } else {
                    "en"
                }
            }
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    pub close_to_tray: bool,
    // None means the pre-tray localStorage language has not been imported yet.
    pub language: Option<Language>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            close_to_tray: true,
            language: None,
        }
    }
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Patch {
    pub close_to_tray: Option<bool>,
    pub language: Option<Language>,
}
pub struct PreferenceStore {
    path: Option<PathBuf>,
    pub value: Preferences,
    pub failed: bool,
    pub unavailable: bool,
}
impl PreferenceStore {
    pub fn load(path: Option<PathBuf>) -> Self {
        let result = path
            .as_ref()
            .map(|p| match std::fs::read(p) {
                Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| ()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound && missing_file_is_valid(p) => {
                    Ok(Preferences::default())
                }
                Err(_) => Err(()),
            })
            .unwrap_or_else(|| Ok(Preferences::default()));
        let failed = result.is_err();
        Self {
            path,
            value: result.unwrap_or(Preferences {
                close_to_tray: false,
                language: None,
            }),
            failed,
            unavailable: false,
        }
    }
    pub fn update(
        &mut self,
        patch: Patch,
        legacy: Option<Language>,
        supported: bool,
    ) -> Result<(), UiError> {
        if self.unavailable {
            return Err(ui("desktopPreferences"));
        }
        if patch.close_to_tray.is_some() && !supported {
            return Err(ui("trayUnsupported"));
        }
        let mut next = self.value.clone();
        if let Some(value) = patch.close_to_tray {
            next.close_to_tray = value;
        }
        if let Some(value) = patch.language {
            next.language = Some(value);
        } else if next.language.is_none() {
            next.language = legacy;
        }
        // Queries and system-locale refreshes do not touch the preference file.
        if next.close_to_tray == self.value.close_to_tray && next.language == self.value.language {
            return Ok(());
        }
        if let Some(path) = &self.path {
            let save = || -> Result<(), Box<dyn std::error::Error>> {
                let parent = path.parent().ok_or("preference directory")?;
                std::fs::create_dir_all(parent)?;
                let mut file = tempfile::NamedTempFile::new_in(parent)?;
                file.write_all(&serde_json::to_vec(&next)?)?;
                file.as_file().sync_all()?;
                file.persist(path)?;
                Ok(())
            };
            save().map_err(|_| ui("desktopPreferences"))?;
        }
        self.value = next;
        self.failed = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn imports_language_once_and_preserves_preferences_across_restart() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("nested").join("preferences.json");
        let mut store = PreferenceStore::load(Some(path.clone()));
        store
            .update(Patch::default(), Some(Language::Chinese), true)
            .unwrap();
        store
            .update(
                Patch {
                    close_to_tray: Some(false),
                    language: None,
                },
                None,
                true,
            )
            .unwrap();
        let mut restored = PreferenceStore::load(Some(path));
        restored
            .update(Patch::default(), Some(Language::English), true)
            .unwrap();
        assert_eq!(restored.value.language, Some(Language::Chinese));
        assert!(!restored.value.close_to_tray);
    }
    #[test]
    fn failed_save_and_unsupported_tray_do_not_change_effective_preferences() {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("file");
        std::fs::write(&parent, b"keep").unwrap();
        let mut store = PreferenceStore::load(Some(parent.join("preferences.json")));
        assert!(store.failed);
        assert!(!store.value.close_to_tray);
        assert!(PreferenceStore::load(Some(parent.join("nested/preferences.json"))).failed);
        assert!(
            store
                .update(
                    Patch {
                        close_to_tray: Some(true),
                        language: None
                    },
                    None,
                    true
                )
                .is_err()
        );
        assert!(!store.value.close_to_tray);
        let mut store = PreferenceStore::load(None);
        assert!(
            store
                .update(
                    Patch {
                        close_to_tray: Some(false),
                        language: None
                    },
                    None,
                    false
                )
                .is_err()
        );
        assert!(store.value.close_to_tray);
    }
}
