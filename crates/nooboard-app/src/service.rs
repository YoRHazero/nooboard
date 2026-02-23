use std::any::Any;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{SystemTime, UNIX_EPOCH};

use nooboard_platform::ClipboardBackend;
use nooboard_storage::{ClipboardRecord, ClipboardRepository, SqliteClipboardRepository};
use nooboard_sync::{SyncConfig, SyncEngine};
use tokio::runtime::{Builder, Runtime};
use tokio::sync::watch;

use crate::{AppError, SyncState, SyncStatus};

type BackendFactory = Arc<dyn Fn() -> Result<Box<dyn ClipboardBackend>, AppError> + Send + Sync>;
type RuntimeFactory = Arc<dyn Fn() -> Result<Runtime, AppError> + Send + Sync>;

#[derive(Debug, Clone)]
pub struct SyncStartConfig {
    pub device_id: String,
    pub listen: SocketAddr,
    pub token: String,
    pub peers: Vec<SocketAddr>,
    pub mdns_enabled: bool,
}

pub trait AppService: Send + Sync {
    /// Return recent clipboard records. `keyword` is trimmed before filtering.
    fn list_history(
        &self,
        limit: usize,
        keyword: Option<&str>,
    ) -> Result<Vec<ClipboardRecord>, AppError>;
    /// Write text to clipboard without creating a history record directly.
    fn set_clipboard(&self, text: &str) -> Result<(), AppError>;
    /// Start sync worker. Returns `SyncAlreadyRunning` when worker is active.
    fn start_sync(&self, config: SyncStartConfig) -> Result<(), AppError>;
    /// Stop sync worker and converge to `Stopped` even when worker is absent.
    fn stop_sync(&self) -> Result<(), AppError>;
    /// Return a cloned sync status snapshot.
    fn sync_status(&self) -> SyncStatus;
}

pub struct AppServiceImpl {
    config_path: PathBuf,
    backend_factory: BackendFactory,
    runtime_factory: RuntimeFactory,
    sync_status: Arc<Mutex<SyncStatus>>,
    sync_worker: Mutex<Option<SyncWorker>>,
}

struct SyncWorker {
    stop_tx: watch::Sender<bool>,
    join_handle: JoinHandle<()>,
}

impl AppServiceImpl {
    pub fn new(config_path: impl Into<PathBuf>) -> Self {
        Self::new_with_backend_and_runtime_factory(
            config_path,
            Arc::new(create_backend),
            Arc::new(build_sync_runtime),
        )
    }

    #[cfg(test)]
    fn new_with_backend_factory(
        config_path: impl Into<PathBuf>,
        backend_factory: BackendFactory,
    ) -> Self {
        Self::new_with_backend_and_runtime_factory(
            config_path,
            backend_factory,
            Arc::new(build_sync_runtime),
        )
    }

    fn new_with_backend_and_runtime_factory(
        config_path: impl Into<PathBuf>,
        backend_factory: BackendFactory,
        runtime_factory: RuntimeFactory,
    ) -> Self {
        Self {
            config_path: config_path.into(),
            backend_factory,
            runtime_factory,
            sync_status: Arc::new(Mutex::new(SyncStatus::default())),
            sync_worker: Mutex::new(None),
        }
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    fn open_repository(&self) -> Result<SqliteClipboardRepository, AppError> {
        open_repository_from_config(&self.config_path)
    }

    fn create_backend(&self) -> Result<Box<dyn ClipboardBackend>, AppError> {
        (self.backend_factory)()
    }

    fn update_status(&self, updater: impl FnOnce(&mut SyncStatus)) {
        let mut status = lock_or_recover(&self.sync_status);
        updater(&mut status);
        status.last_event_at = Some(now_millis());
    }

    fn reap_finished_worker(&self) {
        let finished_worker = {
            let mut worker_slot = lock_or_recover(&self.sync_worker);
            if worker_slot
                .as_ref()
                .is_some_and(|worker| worker.join_handle.is_finished())
            {
                worker_slot.take()
            } else {
                None
            }
        };
        if let Some(worker) = finished_worker {
            let _ = self.join_worker(worker);
        }
    }

    fn join_worker(&self, worker: SyncWorker) -> Result<(), AppError> {
        if let Err(panic_payload) = worker.join_handle.join() {
            let message = panic_payload_message(panic_payload);
            self.update_status(|status| {
                status.state = SyncState::Error;
                status.connected_peers = 0;
                status.last_error = Some(message.clone());
            });
            return Err(AppError::Runtime(message));
        }

        Ok(())
    }
}

impl Default for AppServiceImpl {
    fn default() -> Self {
        Self::new(nooboard_storage::default_dev_config_path())
    }
}

impl AppService for AppServiceImpl {
    fn list_history(
        &self,
        limit: usize,
        keyword: Option<&str>,
    ) -> Result<Vec<ClipboardRecord>, AppError> {
        let repository = self.open_repository()?;
        let keyword = keyword.map(str::trim).filter(|value| !value.is_empty());
        if let Some(value) = keyword {
            repository.search_recent(limit, value).map_err(Into::into)
        } else {
            repository.list_recent(limit).map_err(Into::into)
        }
    }

    fn set_clipboard(&self, text: &str) -> Result<(), AppError> {
        let backend = self.create_backend()?;
        backend.write_text(text).map_err(Into::into)
    }

    fn start_sync(&self, config: SyncStartConfig) -> Result<(), AppError> {
        self.reap_finished_worker();

        let mut worker_slot = lock_or_recover(&self.sync_worker);
        if worker_slot.is_some() {
            return Err(AppError::SyncAlreadyRunning);
        }

        self.update_status(|status| {
            status.state = SyncState::Starting;
            status.listen = Some(config.listen);
            status.connected_peers = 0;
            status.last_error = None;
        });

        let runtime = match (self.runtime_factory)() {
            Ok(runtime) => runtime,
            Err(error) => {
                self.update_status(|status| {
                    status.state = SyncState::Error;
                    status.listen = Some(config.listen);
                    status.connected_peers = 0;
                    status.last_error = Some(error.to_string());
                });
                return Err(error);
            }
        };

        let status = Arc::clone(&self.sync_status);
        let config_path = self.config_path.clone();
        let backend_factory = Arc::clone(&self.backend_factory);
        let (stop_tx, stop_rx) = watch::channel(false);
        let sync_config = SyncConfig {
            device_id: config.device_id,
            listen_addr: config.listen,
            token: config.token,
            peers: config.peers,
            mdns_enabled: config.mdns_enabled,
        };
        let listen_addr = sync_config.listen_addr;
        let peer_status = Arc::clone(&status);

        let join_handle = std::thread::spawn(move || {
            update_status_arc(&status, |sync_status| {
                sync_status.state = SyncState::Running;
                sync_status.listen = Some(listen_addr);
                sync_status.connected_peers = 0;
                sync_status.last_error = None;
            });

            let run_result = runtime.block_on(async move {
                let backend = backend_factory()?;
                let repository = open_repository_from_config(&config_path)?;
                SyncEngine::new(backend.as_ref(), &repository)
                    .run_with_shutdown_and_peer_observer(sync_config, stop_rx, move |connected| {
                        update_status_arc(&peer_status, |sync_status| {
                            sync_status.connected_peers = connected;
                        });
                    })
                    .await
                    .map_err(AppError::from)
            });

            match run_result {
                Ok(()) => {
                    update_status_arc(&status, |sync_status| {
                        sync_status.state = SyncState::Stopped;
                        sync_status.listen = Some(listen_addr);
                        sync_status.connected_peers = 0;
                        sync_status.last_error = None;
                    });
                }
                Err(error) => {
                    update_status_arc(&status, |sync_status| {
                        sync_status.state = SyncState::Error;
                        sync_status.listen = Some(listen_addr);
                        sync_status.connected_peers = 0;
                        sync_status.last_error = Some(error.to_string());
                    });
                }
            }
        });

        *worker_slot = Some(SyncWorker {
            stop_tx,
            join_handle,
        });
        Ok(())
    }

    fn stop_sync(&self) -> Result<(), AppError> {
        let worker = {
            let mut worker_slot = lock_or_recover(&self.sync_worker);
            worker_slot.take()
        };

        if let Some(worker) = worker {
            self.update_status(|status| {
                status.state = SyncState::Stopping;
                status.connected_peers = 0;
                status.last_error = None;
            });
            let _ = worker.stop_tx.send(true);
            self.join_worker(worker)?;
        }

        self.update_status(|status| {
            status.state = SyncState::Stopped;
            status.connected_peers = 0;
            status.last_error = None;
        });
        Ok(())
    }

    fn sync_status(&self) -> SyncStatus {
        self.reap_finished_worker();
        lock_or_recover(&self.sync_status).clone()
    }
}

fn create_backend() -> Result<Box<dyn ClipboardBackend>, AppError> {
    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(
            nooboard_platform_macos::MacOsClipboardBackend::new(),
        ))
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err(AppError::UnsupportedPlatform)
    }
}

fn build_sync_runtime() -> Result<Runtime, AppError> {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| AppError::Runtime(format!("failed to build runtime: {error}")))
}

fn open_repository_from_config(config_path: &Path) -> Result<SqliteClipboardRepository, AppError> {
    let repository = SqliteClipboardRepository::open_from_config(config_path)?;
    repository.init_schema()?;
    Ok(repository)
}

fn lock_or_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn update_status_arc(status: &Arc<Mutex<SyncStatus>>, updater: impl FnOnce(&mut SyncStatus)) {
    let mut guard = lock_or_recover(status);
    updater(&mut guard);
    guard.last_event_at = Some(now_millis());
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

fn panic_payload_message(panic_payload: Box<dyn Any + Send>) -> String {
    panic_payload
        .downcast_ref::<&str>()
        .map(|value| value.to_string())
        .or_else(|| panic_payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "sync worker panicked".to_string())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::thread::{self, JoinHandle};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
    use std::sync::atomic::AtomicBool;

    use nooboard_core::NooboardError;
    use nooboard_platform::ClipboardEventSender;

    use super::*;

    static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(0);

    fn temp_file_path(name: &str, extension: &str) -> PathBuf {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        let test_id = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "nooboard-app-{name}-{millis}-{test_id}.{extension}"
        ))
    }

    fn schema_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("sql")
            .join("schema.sql")
    }

    fn write_temp_config() -> Result<(PathBuf, PathBuf), AppError> {
        let db_path = temp_file_path("test-db", "db");
        let config_path = temp_file_path("test-config", "toml");
        let schema = schema_path();
        let raw = format!(
            "[storage]\ndb_path = \"{}\"\nschema_path = \"{}\"\n",
            db_path.display(),
            schema.display(),
        );
        fs::write(&config_path, raw).map_err(|error| AppError::Runtime(error.to_string()))?;
        Ok((config_path, db_path))
    }

    fn test_sync_config() -> Result<SyncStartConfig, AppError> {
        Ok(SyncStartConfig {
            device_id: "app-test-device".to_string(),
            listen: "127.0.0.1:0"
                .parse()
                .map_err(|error: std::net::AddrParseError| AppError::Runtime(error.to_string()))?,
            token: "dev-token".to_string(),
            peers: Vec::new(),
            mdns_enabled: false,
        })
    }

    fn cleanup_temp_files(config_path: PathBuf, db_path: PathBuf) {
        let _ = fs::remove_file(config_path);
        let _ = fs::remove_file(db_path);
    }

    fn wait_for_status(
        service: &AppServiceImpl,
        timeout: Duration,
        predicate: impl Fn(&SyncStatus) -> bool,
    ) -> Option<SyncStatus> {
        let start = Instant::now();
        loop {
            let status = service.sync_status();
            if predicate(&status) {
                return Some(status);
            }
            if start.elapsed() > timeout {
                return None;
            }
            thread::sleep(Duration::from_millis(20));
        }
    }

    struct IdleBackend;

    impl ClipboardBackend for IdleBackend {
        fn read_text(&self) -> Result<Option<String>, NooboardError> {
            Ok(None)
        }

        fn write_text(&self, _: &str) -> Result<(), NooboardError> {
            Ok(())
        }

        fn watch_changes(
            &self,
            _: ClipboardEventSender,
            shutdown: Arc<AtomicBool>,
            _: std::time::Duration,
        ) -> Result<JoinHandle<()>, NooboardError> {
            Ok(thread::spawn(move || {
                while !shutdown.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_millis(20));
                }
            }))
        }
    }

    struct FailingWriteBackend;

    impl ClipboardBackend for FailingWriteBackend {
        fn read_text(&self) -> Result<Option<String>, NooboardError> {
            Ok(None)
        }

        fn write_text(&self, _: &str) -> Result<(), NooboardError> {
            Err(NooboardError::platform("mock write failure"))
        }

        fn watch_changes(
            &self,
            _: ClipboardEventSender,
            _: Arc<AtomicBool>,
            _: std::time::Duration,
        ) -> Result<JoinHandle<()>, NooboardError> {
            Ok(std::thread::spawn(|| {}))
        }
    }

    #[test]
    fn list_history_supports_keyword_filter() -> Result<(), AppError> {
        let (config_path, db_path) = write_temp_config()?;
        let repository = open_repository_from_config(&config_path)?;
        repository.insert_text_event("alpha", 100)?;
        repository.insert_text_event("beta", 200)?;
        repository.insert_text_event("alphabet", 300)?;

        let service = AppServiceImpl::new(&config_path);
        let records = service.list_history(10, Some("alpha"))?;
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].content, "alphabet");
        assert_eq!(records[1].content, "alpha");

        cleanup_temp_files(config_path, db_path);
        Ok(())
    }

    #[test]
    fn sync_status_switches_between_start_and_stop() -> Result<(), AppError> {
        let (config_path, db_path) = write_temp_config()?;
        let service = AppServiceImpl::new_with_backend_factory(
            &config_path,
            Arc::new(|| Ok(Box::new(IdleBackend))),
        );

        assert_eq!(service.sync_status().state, SyncState::Stopped);

        service.start_sync(test_sync_config()?)?;
        let started = wait_for_status(&service, Duration::from_secs(2), |status| {
            matches!(status.state, SyncState::Running | SyncState::Starting)
        });
        assert!(started.is_some(), "sync should enter starting/running state");
        service.stop_sync()?;
        assert_eq!(service.sync_status().state, SyncState::Stopped);

        cleanup_temp_files(config_path, db_path);
        Ok(())
    }

    #[test]
    fn set_clipboard_maps_backend_write_error() -> Result<(), AppError> {
        let (config_path, db_path) = write_temp_config()?;
        let service = AppServiceImpl::new_with_backend_factory(
            &config_path,
            Arc::new(|| Ok(Box::new(FailingWriteBackend))),
        );

        let error = service
            .set_clipboard("failing write")
            .expect_err("set_clipboard should fail when backend write fails");
        match error {
            AppError::Platform(message) => assert!(message.contains("mock write failure")),
            other => panic!("unexpected error type: {other:?}"),
        }

        cleanup_temp_files(config_path, db_path);
        Ok(())
    }

    #[test]
    fn start_sync_rejects_duplicate_start() -> Result<(), AppError> {
        let (config_path, db_path) = write_temp_config()?;
        let service = AppServiceImpl::new_with_backend_factory(
            &config_path,
            Arc::new(|| Ok(Box::new(IdleBackend))),
        );
        let config = test_sync_config()?;

        service.start_sync(config.clone())?;
        let duplicate_start = service.start_sync(config);
        assert!(matches!(duplicate_start, Err(AppError::SyncAlreadyRunning)));
        service.stop_sync()?;

        cleanup_temp_files(config_path, db_path);
        Ok(())
    }

    #[test]
    fn stop_sync_when_not_running_is_idempotent() -> Result<(), AppError> {
        let (config_path, db_path) = write_temp_config()?;
        let service = AppServiceImpl::new_with_backend_factory(
            &config_path,
            Arc::new(|| Ok(Box::new(IdleBackend))),
        );

        service.stop_sync()?;
        service.stop_sync()?;
        let status = service.sync_status();
        assert_eq!(status.state, SyncState::Stopped);
        assert_eq!(status.connected_peers, 0);
        assert!(status.last_error.is_none());

        cleanup_temp_files(config_path, db_path);
        Ok(())
    }

    #[test]
    fn runtime_build_failure_is_mapped_and_sync_can_restart() -> Result<(), AppError> {
        let (config_path, db_path) = write_temp_config()?;
        let attempts = Arc::new(AtomicU64::new(0));
        let runtime_factory: RuntimeFactory = {
            let attempts = Arc::clone(&attempts);
            Arc::new(move || {
                if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                    Err(AppError::Runtime(
                        "failed to build runtime: mock runtime factory failure".to_string(),
                    ))
                } else {
                    build_sync_runtime()
                }
            })
        };
        let service = AppServiceImpl::new_with_backend_and_runtime_factory(
            &config_path,
            Arc::new(|| Ok(Box::new(IdleBackend))),
            runtime_factory,
        );

        let error = service
            .start_sync(test_sync_config()?)
            .expect_err("first start should fail because runtime factory is mocked to fail");
        match error {
            AppError::Runtime(message) => assert!(message.contains("mock runtime factory failure")),
            other => panic!("unexpected error type: {other:?}"),
        }
        let status = service.sync_status();
        assert_eq!(status.state, SyncState::Error);
        assert!(
            status
                .last_error
                .as_deref()
                .is_some_and(|message| message.contains("mock runtime factory failure"))
        );

        service.start_sync(test_sync_config()?)?;
        assert!(
            wait_for_status(&service, Duration::from_secs(2), |status| {
                matches!(status.state, SyncState::Running | SyncState::Starting)
            })
            .is_some(),
            "sync should be restartable after runtime build failure"
        );
        service.stop_sync()?;

        cleanup_temp_files(config_path, db_path);
        Ok(())
    }

    #[test]
    fn sync_worker_panic_is_mapped_to_runtime_error() -> Result<(), AppError> {
        let (config_path, db_path) = write_temp_config()?;
        let service = AppServiceImpl::new_with_backend_factory(
            &config_path,
            Arc::new(|| -> Result<Box<dyn ClipboardBackend>, AppError> {
                panic!("mock sync worker panic")
            }),
        );

        service.start_sync(test_sync_config()?)?;
        let error = service
            .stop_sync()
            .expect_err("stop_sync should map worker panic to runtime error");
        match error {
            AppError::Runtime(message) => assert!(message.contains("mock sync worker panic")),
            other => panic!("unexpected error type: {other:?}"),
        }
        assert_eq!(service.sync_status().state, SyncState::Error);

        cleanup_temp_files(config_path, db_path);
        Ok(())
    }

    #[test]
    fn sync_can_restart_after_engine_error_or_stop() -> Result<(), AppError> {
        let (config_path, db_path) = write_temp_config()?;
        let attempts = Arc::new(AtomicU64::new(0));
        let service = AppServiceImpl::new_with_backend_factory(
            &config_path,
            Arc::new({
                let attempts = Arc::clone(&attempts);
                move || {
                    if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                        Err(AppError::Platform("mock engine setup failure".to_string()))
                    } else {
                        Ok(Box::new(IdleBackend))
                    }
                }
            }),
        );

        service.start_sync(test_sync_config()?)?;
        assert!(
            wait_for_status(&service, Duration::from_secs(2), |status| {
                status.state == SyncState::Error
            })
            .is_some(),
            "first run should end in error"
        );

        service.start_sync(test_sync_config()?)?;
        assert!(
            wait_for_status(&service, Duration::from_secs(2), |status| {
                matches!(status.state, SyncState::Running | SyncState::Starting)
            })
            .is_some(),
            "second run should succeed after error"
        );
        service.stop_sync()?;
        assert_eq!(service.sync_status().state, SyncState::Stopped);

        service.start_sync(test_sync_config()?)?;
        assert!(
            wait_for_status(&service, Duration::from_secs(2), |status| {
                matches!(status.state, SyncState::Running | SyncState::Starting)
            })
            .is_some(),
            "third run should succeed after explicit stop"
        );
        service.stop_sync()?;

        cleanup_temp_files(config_path, db_path);
        Ok(())
    }

    #[test]
    fn sync_status_exposes_connected_peers_path() -> Result<(), AppError> {
        let (config_path, db_path) = write_temp_config()?;
        let service = AppServiceImpl::new_with_backend_factory(
            &config_path,
            Arc::new(|| Ok(Box::new(IdleBackend))),
        );

        service.start_sync(test_sync_config()?)?;
        let running = wait_for_status(&service, Duration::from_secs(2), |status| {
            matches!(status.state, SyncState::Running | SyncState::Starting)
        })
        .expect("sync should enter starting/running");
        assert_eq!(running.connected_peers, 0);

        service.stop_sync()?;
        let stopped = service.sync_status();
        assert_eq!(stopped.state, SyncState::Stopped);
        assert_eq!(stopped.connected_peers, 0);

        cleanup_temp_files(config_path, db_path);
        Ok(())
    }

    #[test]
    fn last_event_at_updates_on_status_changes() -> Result<(), AppError> {
        let (config_path, db_path) = write_temp_config()?;
        let runtime_factory: RuntimeFactory = Arc::new(|| {
            Err(AppError::Runtime(
                "failed to build runtime: mock last_event_at runtime failure".to_string(),
            ))
        });
        let service = AppServiceImpl::new_with_backend_and_runtime_factory(
            &config_path,
            Arc::new(|| Ok(Box::new(IdleBackend))),
            runtime_factory,
        );

        assert!(service.sync_status().last_event_at.is_none());

        service.stop_sync()?;
        let first = service
            .sync_status()
            .last_event_at
            .expect("stop should set last_event_at");
        thread::sleep(Duration::from_millis(5));

        let _ = service.start_sync(test_sync_config()?).expect_err(
            "start should fail because runtime factory is mocked to fail",
        );
        let second = service
            .sync_status()
            .last_event_at
            .expect("failed start should set last_event_at");
        assert!(second > first);
        thread::sleep(Duration::from_millis(5));

        service.stop_sync()?;
        let third = service
            .sync_status()
            .last_event_at
            .expect("stop should update last_event_at");
        assert!(third > second);

        cleanup_temp_files(config_path, db_path);
        Ok(())
    }
}
