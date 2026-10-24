//! Core ownership and consumer-port implementations. No window or IPC operations.
use crate::{
    error::{Error, Future, Result},
    features::{
        file_selection::ports::SelectedFiles,
        lifecycle::{
            domain::ExitControl,
            ports::{BackendControl, Interaction},
        },
        tray::domain::Summary,
    },
};
use nooboard_core::{App, AppService, BackendConfig, Options, SqliteOptions};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::{Mutex, RwLock, watch};
#[derive(Clone)]
pub struct Connection {
    pub app: App,
    pub diagnostic: bool,
}
#[derive(Clone)]
pub enum State {
    Starting,
    Ready(Connection),
    Failed,
    Stopped,
}
enum Running {
    Native {
        service: AppService,
        app: App,
    },
    #[cfg(all(debug_assertions, feature = "diagnostics", target_os = "macos"))]
    Diagnostic(crate::diagnostics::Diagnostics),
}
impl Running {
    fn connection(&self) -> Connection {
        match self {
            Self::Native { app, .. } => Connection {
                app: app.clone(),
                diagnostic: false,
            },
            #[cfg(all(debug_assertions, feature = "diagnostics", target_os = "macos"))]
            Self::Diagnostic(d) => Connection {
                app: d.local.app.clone(),
                diagnostic: true,
            },
        }
    }
    async fn shutdown(&mut self) -> Result<()> {
        match self {
            Self::Native { service, .. } => service.shutdown().await?,
            #[cfg(all(debug_assertions, feature = "diagnostics", target_os = "macos"))]
            Self::Diagnostic(d) => d.shutdown().await?,
        };
        Ok(())
    }
}
pub struct CoreConfig {
    pub directory: Option<PathBuf>,
    pub downloads: Option<PathBuf>,
    pub diagnostic: bool,
}
pub struct Backend {
    config: CoreConfig,
    running: RwLock<Option<Running>>,
    mutations: Mutex<()>,
    exit: Arc<ExitControl>,
    state: watch::Sender<State>,
}
impl Backend {
    pub fn new(config: CoreConfig, exit: Arc<ExitControl>) -> Self {
        Self {
            config,
            running: RwLock::new(None),
            mutations: Mutex::new(()),
            exit,
            state: watch::channel(State::Starting).0,
        }
    }
    pub fn subscribe(&self) -> watch::Receiver<State> {
        self.state.subscribe()
    }
    pub async fn connection(&self) -> Result<Connection> {
        self.running
            .read()
            .await
            .as_ref()
            .map(Running::connection)
            .ok_or(Error::BackendUnavailable)
    }
    pub async fn app(&self) -> Result<App> {
        Ok(self.connection().await?.app)
    }
    pub async fn ensure_started(&self) -> Result<()> {
        let mut running = self.running.write().await;
        if self.exit.exiting() {
            return Err(Error::Closing);
        }
        if let Some(old) = running.as_mut()
            && !old.connection().app.is_running()
        {
            old.shutdown().await?;
            running.take();
        }
        if running.is_none() {
            self.state.send_replace(State::Starting);
            let backend = match self.start().await {
                Ok(value) => value,
                Err(error) => {
                    self.state.send_replace(State::Failed);
                    return Err(error);
                }
            };
            let connection = backend.connection();
            // Register ownership before yielding to a concurrently requested shutdown.
            *running = Some(backend);
            self.state.send_replace(State::Ready(connection));
            if self.exit.exiting() {
                return Err(Error::Closing);
            }
        }
        Ok(())
    }
    async fn start(&self) -> Result<Running> {
        if self.config.diagnostic {
            #[cfg(all(debug_assertions, feature = "diagnostics", target_os = "macos"))]
            {
                return Ok(Running::Diagnostic(
                    crate::diagnostics::Diagnostics::start().await?,
                ));
            }
            #[cfg(not(all(debug_assertions, feature = "diagnostics", target_os = "macos")))]
            {
                return Err(Error::DiagnosticsDisabled);
            }
        }
        let directory = self.config.directory.as_ref().ok_or(Error::AppDirectory)?;
        std::fs::create_dir_all(directory).map_err(|_| Error::AppDirectory)?;
        let (service, app) = AppService::start(Options {
            storage: BackendConfig::Sqlite(SqliteOptions::file(directory.join("nooboard.sqlite3"))),
            profile: "nooboard.desktop.v1".into(),
            default_receive_directory: self.config.downloads.as_ref().map(|p| p.join("Nooboard")),
        })
        .await?;
        Ok(Running::Native { service, app })
    }
    pub async fn edit_settings(
        &self,
        app: &App,
        edit: impl FnOnce(nooboard_core::Settings) -> Result<nooboard_core::Settings>,
    ) -> Result<u64> {
        let _guard = self.mutations.lock().await;
        app.set_settings(edit(app.status().settings)?).await?;
        Ok(app.status().configuration_revision)
    }
    pub async fn edit_peer(
        &self,
        app: &App,
        id: String,
        edit: impl FnOnce(nooboard_core::PeerSettings) -> nooboard_core::PeerSettings,
    ) -> Result<()> {
        let _guard = self.mutations.lock().await;
        let settings = app
            .status()
            .peers
            .into_iter()
            .find(|p| p.noob_id == id)
            .ok_or(nooboard_core::Error::NotFound)?
            .settings;
        app.configure_peer(id, edit(settings)).await?;
        Ok(())
    }
    pub async fn probe(&self, action: &str) -> Result<()> {
        #[cfg(all(debug_assertions, feature = "diagnostics", target_os = "macos"))]
        if let Some(Running::Diagnostic(d)) = self.running.write().await.as_mut() {
            return d.action(action).await;
        }
        let _ = action;
        Err(Error::DiagnosticsDisabled)
    }
}
impl BackendControl for Backend {
    fn pending_transfers(&self) -> Interaction<'_, usize> {
        Box::pin(async {
            self.app()
                .await
                .map(|app| pending(&app.snapshot()))
                .unwrap_or(0)
        })
    }
    fn shutdown(&self) -> Future<'_, ()> {
        Box::pin(async {
            let mut running = self.running.write().await;
            if let Some(backend) = running.as_mut() {
                backend.shutdown().await?;
            }
            running.take();
            self.state.send_replace(State::Stopped);
            Ok(())
        })
    }
}
pub struct FileTarget<'a> {
    pub backend: &'a Backend,
    pub app: App,
}
impl SelectedFiles for FileTarget<'_> {
    fn send(&self, paths: Vec<PathBuf>) -> Future<'_, ()> {
        Box::pin(async move {
            self.app.send_files(paths).await?;
            Ok(())
        })
    }
    fn set_receive_directory(&self, path: PathBuf) -> Future<'_, ()> {
        Box::pin(async move {
            self.backend
                .edit_settings(&self.app, |mut settings| {
                    settings.receive_directory = Some(path);
                    Ok(settings)
                })
                .await?;
            Ok(())
        })
    }
}
pub fn pending(snapshot: &nooboard_core::AppSnapshot) -> usize {
    snapshot
        .content_transfers
        .iter()
        .filter(|t| t.stage.pending())
        .count()
        + snapshot
            .status
            .transfers
            .iter()
            .filter(|t| t.pending())
            .count()
}
pub fn tray_summary(snapshot: &nooboard_core::AppSnapshot) -> Summary {
    Summary::Running {
        paused: snapshot.status.settings.paused,
        connected: snapshot.status.peers.iter().filter(|p| p.online).count(),
        transfers: pending(snapshot),
    }
}
