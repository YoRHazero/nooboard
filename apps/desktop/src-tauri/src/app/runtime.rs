//! Owns the composed desktop features and their background subscriptions.
use super::{
    adapters::core::{self, Backend, FileTarget},
    snapshot::Snapshot,
};
use crate::{
    error::Result,
    features::{
        file_selection::application::FileSelection,
        lifecycle::application::Lifecycle,
        preferences::{
            application::PreferenceService,
            domain::{Language, Patch},
        },
        tray::{application::TrayService, domain::Summary},
        window::application::WindowService,
    },
    ipc::subscription::Subscriptions,
};
use std::sync::{Arc, Mutex};
use tokio::{sync::watch, task::JoinHandle};
pub struct Runtime {
    pub backend: Arc<Backend>,
    pub window: Arc<WindowService>,
    pub preferences: Arc<PreferenceService>,
    pub tray: Arc<TrayService>,
    pub lifecycle: Arc<Lifecycle>,
    pub subscriptions: Subscriptions,
    pub files: FileSelection,
    state: watch::Sender<Snapshot>,
    tasks: Mutex<Vec<JoinHandle<()>>>,
}
impl Runtime {
    pub fn new(
        backend: Arc<Backend>,
        window: Arc<WindowService>,
        preferences: Arc<PreferenceService>,
        tray: Arc<TrayService>,
        lifecycle: Arc<Lifecycle>,
        files: FileSelection,
    ) -> Self {
        let state = watch::channel(Snapshot {
            revision: 0,
            preferences: preferences.snapshot(),
            window: window.snapshot(),
            tray: tray.snapshot(),
        })
        .0;
        let instance = Self {
            backend,
            window,
            preferences,
            tray,
            lifecycle,
            files,
            subscriptions: Subscriptions::default(),
            state,
            tasks: Mutex::new(vec![]),
        };
        instance.project();
        if instance.tray.snapshot().available {
            instance.watch_backend();
        }
        instance
    }
    pub fn track(&self, task: JoinHandle<()>) {
        self.tasks.lock().unwrap().push(task);
    }
    pub fn snapshot(&self) -> Snapshot {
        self.state.borrow().clone()
    }
    pub fn publish(&self) {
        publish(&self.state, &self.preferences, &self.window, &self.tray);
    }
    pub fn subscribe(&self) -> watch::Receiver<Snapshot> {
        self.state.subscribe()
    }
    pub fn update_preferences(&self, patch: Patch, legacy: Option<Language>) -> Result<Snapshot> {
        self.preferences.update(patch, legacy)?;
        self.publish();
        Ok(self.snapshot())
    }
    pub async fn select_files(&self) -> Result<()> {
        let target = FileTarget {
            backend: &self.backend,
            app: self.backend.app().await?,
        };
        self.files.send_files(&target).await
    }
    pub async fn select_receive_directory(&self) -> Result<()> {
        let target = FileTarget {
            backend: &self.backend,
            app: self.backend.app().await?,
        };
        self.files.receive_directory(&target).await
    }
    fn project(&self) {
        let (mut preferences, mut window) = (self.preferences.subscribe(), self.window.subscribe());
        let state = self.state.clone();
        let tray = self.tray.clone();
        let preference_source = self.preferences.clone();
        let window_source = self.window.clone();
        self.track(tokio::spawn(async move {
            loop {
                tokio::select! {
                    result=preferences.changed()=>if result.is_err(){break;},
                    result=window.changed()=>if result.is_err(){break;},
                }
                preferences.borrow_and_update();
                window.borrow_and_update();
                publish(&state, &preference_source, &window_source, &tray);
            }
        }));
    }
    fn watch_backend(&self) {
        let mut backend = self.backend.subscribe();
        let tray = self.tray.clone();
        self.track(tokio::spawn(async move {
            loop {
                let current = backend.borrow_and_update().clone();
                match current {
                    core::State::Starting => tray.update(Summary::Starting),
                    core::State::Failed | core::State::Stopped => tray.update(Summary::Failed),
                    core::State::Ready(connection) => {
                        let mut snapshots = connection.app.subscribe();
                        loop {
                            tray.update(core::tray_summary(&snapshots.borrow_and_update()));
                            tokio::select! {
                                changed = backend.changed() => {
                                    if changed.is_err() { return; }
                                    break;
                                }
                                changed = snapshots.changed() => {
                                    if changed.is_err() {
                                        tray.update(Summary::Failed);
                                        if backend.changed().await.is_err() { return; }
                                        break;
                                    }
                                }
                            }
                        }
                        continue;
                    }
                }
                if backend.changed().await.is_err() {
                    return;
                }
            }
        }));
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        for task in self.tasks.get_mut().unwrap() {
            task.abort();
        }
    }
}

fn publish(
    state: &watch::Sender<Snapshot>,
    preferences: &PreferenceService,
    window: &WindowService,
    tray: &TrayService,
) {
    // Read the sources while holding the projection write lock. A delayed observer
    // cannot overwrite a newer synchronous reply with an older captured state.
    state.send_modify(|s| {
        s.revision = s.revision.wrapping_add(1);
        s.preferences = preferences.snapshot();
        tray.refresh(&s.preferences.resolved_language);
        s.window = window.snapshot();
        s.tray = tray.snapshot();
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        error::Future,
        features::{
            file_selection::ports::FilePicker,
            lifecycle::{
                domain::ExitControl,
                ports::{ExitInteraction, Interaction},
            },
            preferences::{
                domain::Preferences,
                ports::{PreferenceRepository, SystemLocale},
            },
            tray::{
                domain::{Presentation, State as TrayState},
                ports::TrayPort,
            },
            window::ports::WindowPort,
        },
    };
    use std::path::PathBuf;
    struct Memory;
    impl PreferenceRepository for Memory {
        fn load(&self) -> Result<Preferences> {
            Ok(Preferences::default())
        }
        fn save(&self, _: &Preferences) -> Result<()> {
            Ok(())
        }
    }
    impl SystemLocale for Memory {
        fn locale(&self) -> String {
            "en".into()
        }
    }
    impl WindowPort for Memory {
        fn show(&self) -> bool {
            true
        }
        fn hide(&self) -> bool {
            true
        }
    }
    impl TrayPort for Memory {
        fn state(&self) -> TrayState {
            TrayState {
                supported: true,
                available: true,
            }
        }
        fn render(&self, _: Presentation) {}
    }
    impl FilePicker for Memory {
        fn files(&self) -> Future<'_, Option<Vec<PathBuf>>> {
            Box::pin(async { Ok(None) })
        }
        fn directory(&self) -> Future<'_, Option<PathBuf>> {
            Box::pin(async { Ok(None) })
        }
    }
    impl ExitInteraction for Memory {
        fn confirm(&self, _: usize) -> Interaction<'_, bool> {
            Box::pin(async { false })
        }
        fn failed(&self) -> Interaction<'_, ()> {
            Box::pin(async {})
        }
        fn exit(&self) {
            panic!("projection must not exit")
        }
    }
    #[tokio::test]
    async fn delayed_projection_preserves_latest_preferences_visibility_and_monotonic_revisions() {
        let control = Arc::new(ExitControl::default());
        let backend = Arc::new(Backend::new(
            core::CoreConfig {
                directory: None,
                downloads: None,
                diagnostic: false,
            },
            control.clone(),
        ));
        let window = Arc::new(WindowService::new(Arc::new(Memory)));
        let preferences = Arc::new(PreferenceService::load(
            Arc::new(Memory),
            Arc::new(Memory),
            true,
        ));
        let tray = Arc::new(TrayService::new(Arc::new(Memory), "en".into()));
        let lifecycle = Arc::new(Lifecycle::new(
            control,
            backend.clone(),
            Arc::new(Memory),
            true,
        ));
        let runtime = Runtime::new(
            backend,
            window,
            preferences,
            tray,
            lifecycle,
            FileSelection::new(Arc::new(Memory)),
        );
        let mut changes = runtime.subscribe();
        runtime.window.hide();
        let first = runtime
            .update_preferences(
                Patch {
                    language: Some(Language::Chinese),
                    close_to_tray: None,
                },
                None,
            )
            .unwrap();
        runtime.window.show(None);
        let second = runtime
            .update_preferences(
                Patch {
                    language: Some(Language::English),
                    close_to_tray: None,
                },
                None,
            )
            .unwrap();
        assert!(second.revision > first.revision);
        assert!(second.window.visible);
        changes.borrow_and_update();
        tokio::time::timeout(std::time::Duration::from_secs(3), changes.changed())
            .await
            .unwrap()
            .unwrap();
        let current = runtime.snapshot();
        assert!(current.revision > second.revision);
        assert_eq!(current.preferences.language, Language::English);
        assert!(current.window.visible);
    }
}
