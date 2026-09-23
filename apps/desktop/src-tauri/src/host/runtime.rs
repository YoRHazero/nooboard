//! Owns one backend and one window subscription. Commands never create clipboard workers.
use crate::{
    desktop::{Desktop, tray},
    ipc::{dto, errors},
};
use nooboard_core::{App, AppService, BackendConfig, Options, SqliteOptions};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use tauri::{AppHandle, Manager, ipc::Channel};
use tokio::sync::{Mutex as AsyncMutex, RwLock};

pub enum Running {
    Native {
        service: AppService,
        app: App,
    },
    #[cfg(all(debug_assertions, feature = "diagnostics", target_os = "macos"))]
    Diagnostic(crate::diagnostics::Diagnostics),
}
impl Running {
    pub fn app(&self) -> &App {
        match self {
            Self::Native { app, .. } => app,
            #[cfg(all(debug_assertions, feature = "diagnostics", target_os = "macos"))]
            Self::Diagnostic(diagnostic) => &diagnostic.local.app,
        }
    }
    pub fn diagnostic(&self) -> bool {
        !matches!(self, Self::Native { .. })
    }
    async fn shutdown(self) {
        match self {
            Self::Native { service, .. } => {
                let _ = service.shutdown().await;
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
    subscription_sequence: AtomicU64,
    pump: Mutex<Option<super::subscription::Subscription>>,
    tray_pump: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    pub closing: AtomicBool,
}
impl Host {
    pub async fn app(&self) -> Result<App, errors::UiError> {
        self.running
            .read()
            .await
            .as_ref()
            .map(|running| running.app().clone())
            .ok_or(errors::ui("backendNotConnected"))
    }

    /// Also used by explicit frontend retries; the write lock guarantees one backend.
    pub async fn ensure_started(&self, handle: &AppHandle) -> Result<(), errors::UiError> {
        let mut running = self.running.write().await;
        if self.closing.load(Ordering::SeqCst) {
            return Err(errors::ui("closing"));
        }
        if running.as_ref().is_some_and(|r| !r.app().is_running())
            && let Some(old) = running.take()
        {
            old.shutdown().await;
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
        subscription: String,
        channel: Channel<dto::Frame>,
    ) -> Result<dto::Connection, errors::UiError> {
        if self.closing.load(Ordering::SeqCst) {
            return Err(errors::ui("closing"));
        }
        let sequence = self.subscription_sequence.fetch_add(1, Ordering::SeqCst) + 1;
        self.ensure_started(handle).await?;
        let running = self.running.read().await;
        let backend = running.as_ref().ok_or(errors::ui("backendNotConnected"))?;
        let (connection, pump) = super::subscription::start(
            subscription,
            backend.app().subscribe(),
            handle.state::<Desktop>().state.subscribe(),
            backend.diagnostic(),
            channel,
        );
        let mut active = self.pump.lock().map_err(|_| errors::ui("subscription"))?;
        if self.subscription_sequence.load(Ordering::SeqCst) != sequence {
            return Err(errors::ui("subscription"));
        }
        *active = Some(pump);
        Ok(connection)
    }
    pub fn disconnect(&self, subscription: &str) {
        if let Ok(mut active) = self.pump.lock()
            && active.as_ref().is_some_and(|pump| pump.id == subscription)
        {
            *active = None;
        }
    }
    async fn start(handle: &AppHandle) -> Result<Running, crate::ipc::errors::UiError> {
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
            .map_err(|_| crate::ipc::errors::ui("appDirectory"))?;
        std::fs::create_dir_all(&directory).map_err(|_| crate::ipc::errors::ui("appDirectory"))?;
        AppService::start(Options {
            storage: BackendConfig::Sqlite(SqliteOptions::file(directory.join("nooboard.sqlite3"))),
            profile: "nooboard.desktop.v1".into(),
            default_receive_directory: handle
                .path()
                .download_dir()
                .ok()
                .map(|p| p.join("Nooboard")),
        })
        .await
        .map(|(service, app)| Running::Native { service, app })
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
            drop(task);
        }
        if let Some(backend) = self.running.write().await.take() {
            backend.shutdown().await;
        }
    }
}
