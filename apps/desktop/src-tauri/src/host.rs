//! Owns one backend and one window subscription. Commands never create clipboard workers.
use crate::{errors, wire};
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
    pub closing: AtomicBool,
}
impl Host {
    pub async fn connect(
        &self,
        handle: &AppHandle,
        channel: Channel<wire::Frame>,
    ) -> Result<wire::Connection, crate::errors::UiError> {
        if self.closing.load(Ordering::SeqCst) {
            return Err(crate::errors::ui("closing"));
        }
        let mut running = self.running.write().await;
        if running.as_ref().is_some_and(|r| !r.app().is_running()) {
            *running = None;
        }
        if running.is_none() {
            *running = Some(Self::start(handle).await?);
        }
        let backend = running.as_ref().expect("started backend");
        let mut receiver = backend.app().subscribe_snapshots();
        let initial = wire::snapshot(receiver.borrow_and_update().clone());
        let diagnostic = backend.diagnostic();
        let mut pump = self
            .pump
            .lock()
            .map_err(|_| crate::errors::ui("subscription"))?;
        if let Some(old) = pump.take() {
            old.abort();
        }
        *pump = Some(tauri::async_runtime::spawn(async move {
            while receiver.changed().await.is_ok() {
                let next = wire::snapshot(receiver.borrow_and_update().clone());
                if channel.send(wire::Frame::Snapshot(next)).is_err() {
                    return;
                }
            }
            let _ = channel.send(wire::Frame::Stopped(crate::errors::ui("stopped")));
        }));
        Ok(wire::Connection {
            snapshot: initial,
            diagnostic,
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
        })
        .await
        .map(Running::Native)
        .map_err(errors::core)
    }
    pub async fn shutdown(&self) {
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
