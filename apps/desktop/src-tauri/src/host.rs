//! Owns one backend and one window subscription. Commands never create clipboard workers.
use crate::{
    desktop::{Desktop, tray},
    errors, wire,
};
use nooboard_core::{App, Options};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};
use tauri::{AppHandle, Manager, ipc::Channel};
use tokio::sync::{Mutex as AsyncMutex, RwLock};

pub enum Running {
    Native(App),
    #[cfg(all(debug_assertions, feature = "diagnostics", target_os = "macos"))]
    Diagnostic(crate::diagnostics::Diagnostics),
}
impl Running {
    pub fn app(&self) -> &App {
        match self {
            Self::Native(app) => app,
            #[cfg(all(debug_assertions, feature = "diagnostics", target_os = "macos"))]
            Self::Diagnostic(diagnostic) => &diagnostic.local.app,
        }
    }
    pub fn diagnostic(&self) -> bool {
        !matches!(self, Self::Native(_))
    }
    async fn shutdown(self) {
        match self {
            Self::Native(app) => {
                let _ = app.shutdown().await;
            }
            #[cfg(all(debug_assertions, feature = "diagnostics", target_os = "macos"))]
            Self::Diagnostic(diagnostic) => diagnostic.shutdown().await,
        }
    }
}
#[derive(Default)]
pub struct Host {
    pub running: RwLock<Option<Running>>,
    pub mutations: AsyncMutex<()>,
    pump: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    tray_pump: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    pub closing: AtomicBool,
}
impl Host {
    /// Also used by explicit frontend retries; the write lock guarantees one backend.
    pub async fn ensure_started(&self, handle: &AppHandle) -> Result<(), errors::UiError> {
        let mut running = self.running.write().await;
        if self.closing.load(Ordering::SeqCst) {
            return Err(errors::ui("closing"));
        }
        if running.as_ref().is_some_and(|r| !r.app().is_running()) {
            *running = None;
        }
        if running.is_none() {
            let backend = Self::start(handle)
                .await
                .inspect_err(|_| tray::update(handle, None))?;
            if self.closing.load(Ordering::SeqCst) {
                backend.shutdown().await;
                return Err(errors::ui("closing"));
            }
            // Tray state lives independently of the WebView's subscription.
            if crate::desktop::TRAY_SUPPORTED {
                let mut receiver = backend.app().subscribe_snapshots();
                tray::update(handle, Some(&receiver.borrow_and_update()));
                let handle = handle.clone();
                let mut pump = self
                    .tray_pump
                    .lock()
                    .map_err(|_| errors::ui("subscription"))?;
                if let Some(old) = pump.take() {
                    old.abort();
                }
                *pump = Some(tauri::async_runtime::spawn(async move {
                    while receiver.changed().await.is_ok() {
                        tray::update(&handle, Some(&receiver.borrow_and_update()));
                    }
                    tray::update(&handle, None);
                }));
            }
            *running = Some(backend);
        }
        Ok(())
    }
    pub async fn connect(
        &self,
        handle: &AppHandle,
        channel: Channel<wire::Frame>,
    ) -> Result<wire::Connection, crate::errors::UiError> {
        if self.closing.load(Ordering::SeqCst) {
            return Err(crate::errors::ui("closing"));
        }
        self.ensure_started(handle).await?;
        let running = self.running.read().await;
        let backend = running.as_ref().expect("started backend");
        let mut receiver = backend.app().subscribe_snapshots();
        let initial = wire::snapshot(receiver.borrow_and_update().clone());
        let diagnostic = backend.diagnostic();
        let mut desktop = handle.state::<Desktop>().state.subscribe();
        let initial_desktop = desktop.borrow_and_update().clone();
        let mut visible = initial_desktop.visible;
        let mut pump = self
            .pump
            .lock()
            .map_err(|_| crate::errors::ui("subscription"))?;
        if let Some(old) = pump.take() {
            old.abort();
        }
        *pump = Some(tauri::async_runtime::spawn(async move {
            loop {
                tokio::select! {
                    changed = receiver.changed() => {
                        if changed.is_err() { break; }
                        let current = receiver.borrow_and_update().clone();
                        if visible && channel.send(wire::Frame::Snapshot(wire::snapshot(current))).is_err() { return; }
                    }
                    changed = desktop.changed() => {
                        if changed.is_err() { return; }
                        let state = desktop.borrow_and_update().clone();
                        if state.visible && !visible {
                            // Recover current state without replaying hidden-window animation events.
                            if channel.send(wire::Frame::Recovered(wire::snapshot(receiver.borrow_and_update().clone()))).is_err() { return; }
                        }
                        visible = state.visible;
                        if channel.send(wire::Frame::Desktop(state)).is_err() { return; }
                    }
                }
            }
            let _ = channel.send(wire::Frame::Stopped(crate::errors::ui("stopped")));
        }));
        Ok(wire::Connection {
            snapshot: initial,
            diagnostic,
            desktop: initial_desktop,
        })
    }
    async fn start(handle: &AppHandle) -> Result<Running, crate::errors::UiError> {
        #[cfg(all(debug_assertions, feature = "diagnostics", target_os = "macos"))]
        if std::env::var("NOOBOARD_DIAGNOSTIC").as_deref() == Ok("1") {
            return crate::diagnostics::Diagnostics::start()
                .await
                .map(Running::Diagnostic)
                .map_err(errors::core);
        }
        let directory = handle
            .path()
            .app_data_dir()
            .map_err(|_| crate::errors::ui("appDirectory"))?;
        std::fs::create_dir_all(&directory).map_err(|_| crate::errors::ui("appDirectory"))?;
        App::start(Options {
            database: directory.join("nooboard.sqlite3"),
            profile: "nooboard.desktop.v1".into(),
            default_receive_directory: handle
                .path()
                .download_dir()
                .ok()
                .map(|p| p.join("Nooboard")),
        })
        .await
        .map(Running::Native)
        .map_err(errors::core)
    }
    pub async fn shutdown(&self) {
        if let Ok(mut pump) = self.tray_pump.lock()
            && let Some(task) = pump.take()
        {
            task.abort();
        }
        if let Ok(mut pump) = self.pump.lock()
            && let Some(task) = pump.take()
        {
            task.abort();
        }
        if let Some(backend) = self.running.write().await.take() {
            backend.shutdown().await;
        }
    }
}
