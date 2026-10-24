use super::{
    domain::Preferences,
    ports::{PreferenceRepository, SystemLocale},
};
use crate::error::{Error, Result};
use std::{
    io::Write,
    path::{Path, PathBuf},
};
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

pub struct JsonPreferences {
    pub path: Option<PathBuf>,
    /// A missing application directory fails writes; diagnostic memory storage does not.
    pub unavailable: bool,
}
impl PreferenceRepository for JsonPreferences {
    fn available(&self) -> bool {
        !self.unavailable
    }
    fn load(&self) -> Result<Preferences> {
        if self.unavailable {
            return Err(Error::Preferences);
        }
        match &self.path {
            None => Ok(Preferences::default()),
            Some(path) => match std::fs::read(path) {
                Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| Error::Preferences),
                Err(error)
                    if error.kind() == std::io::ErrorKind::NotFound
                        && missing_file_is_valid(path) =>
                {
                    Ok(Preferences::default())
                }
                Err(_) => Err(Error::Preferences),
            },
        }
    }
    fn save(&self, value: &Preferences) -> Result<()> {
        if self.unavailable {
            return Err(Error::Preferences);
        }
        if let Some(path) = &self.path {
            let save = || -> std::result::Result<(), Box<dyn std::error::Error>> {
                let parent = path.parent().ok_or("preference directory")?;
                std::fs::create_dir_all(parent)?;
                let mut file = tempfile::NamedTempFile::new_in(parent)?;
                file.write_all(&serde_json::to_vec(value)?)?;
                file.as_file().sync_all()?;
                file.persist(path)?;
                Ok(())
            };
            save().map_err(|_| Error::Preferences)?;
        }
        Ok(())
    }
}
pub struct NativeLocale;
impl SystemLocale for NativeLocale {
    fn locale(&self) -> String {
        tauri_plugin_os::locale().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::preferences::{
        application::PreferenceService,
        domain::{Language, Patch},
        ports::SystemLocale,
    };
    use std::sync::Arc;
    struct Locale;
    impl SystemLocale for Locale {
        fn locale(&self) -> String {
            "en".into()
        }
    }
    fn load(path: Option<PathBuf>, supported: bool) -> PreferenceService {
        PreferenceService::load(
            Arc::new(JsonPreferences {
                path,
                unavailable: false,
            }),
            Arc::new(Locale),
            supported,
        )
    }
    #[test]
    fn imports_language_once_and_preserves_preferences_across_restart() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("nested").join("preferences.json");
        let store = load(Some(path.clone()), true);
        store
            .update(Patch::default(), Some(Language::Chinese))
            .unwrap();
        store
            .update(
                Patch {
                    close_to_tray: Some(false),
                    language: None,
                },
                None,
            )
            .unwrap();
        let restored = load(Some(path), true);
        restored
            .update(Patch::default(), Some(Language::English))
            .unwrap();
        assert_eq!(restored.snapshot().language, Language::Chinese);
        assert!(!restored.snapshot().close_to_tray);
    }
    #[test]
    fn failed_save_and_unsupported_tray_do_not_change_effective_preferences() {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("file");
        std::fs::write(&parent, b"keep").unwrap();
        let store = load(Some(parent.join("preferences.json")), true);
        assert!(store.snapshot().failed);
        assert!(!store.snapshot().close_to_tray);
        assert!(
            load(Some(parent.join("nested/preferences.json")), true)
                .snapshot()
                .failed
        );
        assert!(
            store
                .update(
                    Patch {
                        close_to_tray: Some(true),
                        language: None
                    },
                    None,
                )
                .is_err()
        );
        assert!(!store.snapshot().close_to_tray);
        let store = load(None, false);
        assert!(
            store
                .update(
                    Patch {
                        close_to_tray: Some(false),
                        language: None
                    },
                    None,
                )
                .is_err()
        );
        assert!(store.snapshot().close_to_tray);
    }
}
