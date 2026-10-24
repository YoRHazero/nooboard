use super::{
    domain::{Language, Patch, Preferences, State},
    ports::{PreferenceRepository, SystemLocale},
};
use crate::error::{Error, Result};
use std::sync::{Arc, Mutex};
use tokio::sync::watch;
struct Stored {
    value: Preferences,
    failed: bool,
}
pub struct PreferenceService {
    stored: Mutex<Stored>,
    repository: Arc<dyn PreferenceRepository>,
    locale: Arc<dyn SystemLocale>,
    supported: bool,
    state: watch::Sender<State>,
}
impl PreferenceService {
    pub fn load(
        repository: Arc<dyn PreferenceRepository>,
        locale: Arc<dyn SystemLocale>,
        supported: bool,
    ) -> Self {
        let loaded = repository.load();
        let failed = loaded.is_err();
        let value = loaded.unwrap_or(Preferences {
            close_to_tray: false,
            language: None,
        });
        let state = Self::view(&value, failed, supported, locale.as_ref());
        Self {
            stored: Mutex::new(Stored { value, failed }),
            repository,
            locale,
            supported,
            state: watch::channel(state).0,
        }
    }
    fn view(
        value: &Preferences,
        failed: bool,
        _supported: bool,
        locale: &dyn SystemLocale,
    ) -> State {
        let language = value.language.unwrap_or_default();
        State {
            close_to_tray: value.close_to_tray && !failed,
            language,
            resolved_language: language.resolved(&locale.locale()).into(),
            failed,
        }
    }
    pub fn snapshot(&self) -> State {
        self.state.borrow().clone()
    }
    pub fn subscribe(&self) -> watch::Receiver<State> {
        self.state.subscribe()
    }
    pub fn update(&self, patch: Patch, legacy: Option<Language>) -> Result<State> {
        if !self.repository.available() {
            return Err(Error::Preferences);
        }
        if patch.close_to_tray.is_some() && !self.supported {
            return Err(Error::TrayUnsupported);
        }
        let mut stored = self.stored.lock().map_err(|_| Error::Preferences)?;
        let mut next = stored.value.clone();
        if let Some(value) = patch.close_to_tray {
            next.close_to_tray = value;
        }
        if let Some(value) = patch.language {
            next.language = Some(value);
        } else if next.language.is_none() {
            next.language = legacy;
        }
        if next.close_to_tray != stored.value.close_to_tray
            || next.language != stored.value.language
        {
            self.repository.save(&next)?;
            stored.value = next;
            stored.failed = false;
        }
        let state = Self::view(
            &stored.value,
            stored.failed,
            self.supported,
            self.locale.as_ref(),
        );
        self.state.send_replace(state.clone());
        Ok(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[derive(Default)]
    struct Repository {
        writes: AtomicUsize,
        fail: bool,
        available: bool,
    }
    impl PreferenceRepository for Repository {
        fn available(&self) -> bool {
            self.available
        }
        fn load(&self) -> Result<Preferences> {
            Ok(Preferences::default())
        }
        fn save(&self, _: &Preferences) -> Result<()> {
            self.writes.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                Err(Error::Preferences)
            } else {
                Ok(())
            }
        }
    }
    struct Locale;
    impl SystemLocale for Locale {
        fn locale(&self) -> String {
            "zh_TW".into()
        }
    }
    #[test]
    fn queries_do_not_write_and_failed_saves_never_publish() {
        let repository = Arc::new(Repository {
            available: true,
            fail: true,
            ..Default::default()
        });
        let service = PreferenceService::load(repository.clone(), Arc::new(Locale), true);
        let mut changes = service.subscribe();
        assert_eq!(
            service
                .update(Patch::default(), None)
                .unwrap()
                .resolved_language,
            "zh-CN"
        );
        changes.borrow_and_update();
        assert_eq!(repository.writes.load(Ordering::SeqCst), 0);
        assert!(
            service
                .update(
                    Patch {
                        language: Some(Language::English),
                        close_to_tray: None
                    },
                    None
                )
                .is_err()
        );
        assert!(!changes.has_changed().unwrap());
        assert_eq!(service.snapshot().language, Language::System);
    }
    #[test]
    fn unavailable_repository_rejects_even_noop_updates() {
        let service =
            PreferenceService::load(Arc::new(Repository::default()), Arc::new(Locale), true);
        assert!(service.update(Patch::default(), None).is_err());
    }
}
