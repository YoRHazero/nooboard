mod config;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use clap::{Parser, Subcommand};
use config::load_sync_config;
use nooboard_core::{ClipboardEvent, NooboardError};
use nooboard_platform::{ClipboardBackend, DEFAULT_WATCH_INTERVAL};
use nooboard_storage::{SqliteEventRepository, StorageError, default_dev_config_path};
use nooboard_sync::{
    FileDecisionInput, SendFileRequest, SendTextRequest, SyncEngineHandle, SyncEvent, SyncStatus,
    TransferState, start_sync_engine,
};
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

#[derive(Debug, Parser)]
#[command(
    name = "nooboard-cli",
    version,
    about = "Nooboard stage3 clipboard + sync CLI"
)]
struct Cli {
    /// Config file path
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Read text from clipboard
    Get,
    /// Write text into clipboard
    Set {
        /// Text to write into clipboard
        text: String,
    },
    /// Watch clipboard text changes
    Watch {
        /// Poll interval in milliseconds
        #[arg(long, default_value_t = DEFAULT_WATCH_INTERVAL.as_millis() as u64)]
        interval_ms: u64,
    },
    /// Query persisted clipboard history
    History {
        /// Maximum records to display
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Optional keyword filter
        #[arg(long)]
        keyword: Option<String>,
    },
    /// Send a file to currently connected peers via sync engine
    SendFile {
        /// Path to file
        path: PathBuf,
        /// Time to wait before shutdown after dispatch
        #[arg(long, default_value_t = 1500)]
        wait_ms: u64,
    },
}

#[tokio::main]
async fn main() {
    init_tracing();

    if let Err(error) = run().await {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), NooboardError> {
    let cli = Cli::parse();
    let config_path = cli.config.unwrap_or_else(default_dev_config_path);

    match cli.command {
        Commands::Get => {
            let backend = create_backend()?;
            handle_get(backend.as_ref())
        }
        Commands::Set { text } => {
            let backend = create_backend()?;
            handle_set(backend.as_ref(), text)
        }
        Commands::Watch { interval_ms } => {
            let backend = create_backend()?;
            let sync_config = load_sync_config(&config_path)?;
            handle_watch(backend.as_ref(), interval_ms, &config_path, sync_config).await
        }
        Commands::History { limit, keyword } => handle_history(limit, keyword, &config_path),
        Commands::SendFile { path, wait_ms } => handle_send_file(path, wait_ms, &config_path).await,
    }
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}

fn create_backend() -> Result<Box<dyn ClipboardBackend>, NooboardError> {
    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(
            nooboard_platform_macos::MacOsClipboardBackend::new(),
        ))
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err(NooboardError::UnsupportedPlatform)
    }
}

fn open_repository(config_path: &Path) -> Result<SqliteEventRepository, NooboardError> {
    let mut repository =
        SqliteEventRepository::open_from_config(config_path).map_err(storage_error_to_core)?;
    repository.init_storage().map_err(storage_error_to_core)?;
    Ok(repository)
}

fn storage_error_to_core(error: StorageError) -> NooboardError {
    NooboardError::storage(error.to_string())
}

fn handle_get(backend: &dyn ClipboardBackend) -> Result<(), NooboardError> {
    match backend.read_text()? {
        Some(text) => println!("{text}"),
        None => println!("clipboard is empty or does not contain UTF-8 plain text"),
    }

    Ok(())
}

fn handle_set(backend: &dyn ClipboardBackend, text: String) -> Result<(), NooboardError> {
    backend.write_text(&text)?;
    println!("clipboard updated");
    Ok(())
}

fn handle_history(
    limit: usize,
    keyword: Option<String>,
    config_path: &Path,
) -> Result<(), NooboardError> {
    let repository = open_repository(config_path)?;

    let records = match keyword {
        Some(ref value) if !value.trim().is_empty() => repository
            .search_history(limit, value)
            .map_err(storage_error_to_core)?,
        _ => repository
            .list_history(limit, None)
            .map_err(storage_error_to_core)?,
    };

    if records.is_empty() {
        println!("no history records");
        return Ok(());
    }

    for record in records {
        println!(
            "[{}] [{}] {}",
            record.created_at_ms, record.origin_device_id, record.content
        );
    }

    Ok(())
}

async fn handle_watch(
    backend: &dyn ClipboardBackend,
    interval_ms: u64,
    config_path: &Path,
    sync_config: Option<nooboard_sync::SyncConfig>,
) -> Result<(), NooboardError> {
    let mut repository = open_repository(config_path)?;

    let (sender, mut receiver) = mpsc::channel::<ClipboardEvent>(64);
    let shutdown = Arc::new(AtomicBool::new(false));
    let interval = Duration::from_millis(interval_ms.max(1));

    let observer = backend.watch_changes(sender, shutdown.clone(), interval)?;

    let mut sync_handle = if let Some(config) = sync_config {
        let mut handle = start_sync_engine(config)
            .await
            .map_err(|error| NooboardError::storage(error.to_string()))?;
        wait_sync_running(&mut handle.status_rx).await?;
        Some(handle)
    } else {
        None
    };
    let mut sync_progress_rx = sync_handle
        .as_mut()
        .map(|handle| handle.progress_rx.resubscribe());

    println!(
        "watching clipboard changes (interval={}ms). Press Ctrl+C to stop.",
        interval_ms.max(1)
    );

    let mut watch_error: Option<NooboardError> = None;
    let mut suppress_text_once: Option<String> = None;

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("received Ctrl+C, stopping watcher");
                shutdown.store(true, Ordering::Relaxed);
                break;
            }
            maybe_event = receiver.recv() => {
                match maybe_event {
                    Some(event) => {
                        if suppress_text_once.as_deref() == Some(event.text.as_str()) {
                            suppress_text_once = None;
                            continue;
                        }

                        println!("[{}] {}", event.timestamp_millis(), event.text);

                        if let Err(error) = persist_event(&mut repository, &event) {
                            shutdown.store(true, Ordering::Relaxed);
                            watch_error = Some(error);
                            break;
                        }

                        if let Some(handle) = sync_handle.as_ref() {
                            if let Err(error) = handle
                                .text_tx
                                .send(SendTextRequest {
                                    event_id: Uuid::now_v7().to_string(),
                                    content: event.text.clone(),
                                    targets: None,
                                })
                                .await
                            {
                                watch_error = Some(NooboardError::channel(format!(
                                    "failed to send text to sync engine: {error}"
                                )));
                                shutdown.store(true, Ordering::Relaxed);
                                break;
                            }
                        }
                    }
                    None => {
                        shutdown.store(true, Ordering::Relaxed);
                        break;
                    }
                }
            }
            maybe_sync_event = recv_sync_event(&mut sync_handle), if sync_handle.is_some() => {
                match maybe_sync_event {
                    Some(SyncEvent::TextReceived(text)) => {
                        if let Err(error) = backend.write_text(&text) {
                            watch_error = Some(error);
                            shutdown.store(true, Ordering::Relaxed);
                            break;
                        }

                        suppress_text_once = Some(text.clone());
                        let event = ClipboardEvent::new(text.clone());
                        if let Err(error) = persist_event(&mut repository, &event) {
                            watch_error = Some(error);
                            shutdown.store(true, Ordering::Relaxed);
                            break;
                        }

                        println!("[remote] {text}");
                    }
                    Some(SyncEvent::FileDecisionRequired { peer_node_id, transfer_id, file_name, file_size, total_chunks }) => {
                        println!(
                            "[file] incoming from {}: {} ({} bytes, {} chunks)",
                            peer_node_id, file_name, file_size, total_chunks
                        );

                        if let Some(handle) = sync_handle.as_ref() {
                            let decision_tx = handle.decision_tx.clone();
                            tokio::spawn(async move {
                                let (accept, reason) = prompt_file_decision(&peer_node_id, &file_name, file_size).await;
                                let _ = decision_tx.send(FileDecisionInput {
                                    peer_node_id,
                                    transfer_id,
                                    accept,
                                    reason,
                                }).await;
                            });
                        }
                    }
                    Some(SyncEvent::TransferUpdate(update)) => {
                        match update.state {
                            TransferState::Started { file_name, total_bytes } => {
                                println!(
                                    "[file] {:?} transfer {} with {} started: {} ({} bytes)",
                                    update.direction,
                                    update.transfer_id,
                                    update.peer_node_id,
                                    file_name,
                                    total_bytes
                                );
                            }
                            TransferState::Finished { path } => {
                                match path {
                                    Some(path) => println!(
                                        "[file] downloaded {} from {}",
                                        path.display(),
                                        update.peer_node_id
                                    ),
                                    None => println!(
                                        "[file] upload {} to {} finished",
                                        update.transfer_id,
                                        update.peer_node_id
                                    ),
                                }
                            }
                            TransferState::Failed { reason } => {
                                println!(
                                    "[file] transfer {} with {} failed: {}",
                                    update.transfer_id, update.peer_node_id, reason
                                );
                            }
                            TransferState::Cancelled { reason } => {
                                println!(
                                    "[file] transfer {} with {} cancelled{}",
                                    update.transfer_id,
                                    update.peer_node_id,
                                    reason
                                        .as_deref()
                                        .map(|value| format!(": {value}"))
                                        .unwrap_or_default()
                                );
                            }
                            TransferState::Progress { .. } => {}
                        }
                    }
                    Some(SyncEvent::ConnectionError { peer_node_id, addr, error }) => {
                        match (peer_node_id, addr) {
                            (Some(peer), Some(addr)) => {
                                println!("[sync-error] peer={} addr={} error={}", peer, addr, error);
                            }
                            (Some(peer), None) => {
                                println!("[sync-error] peer={} error={}", peer, error);
                            }
                            (None, Some(addr)) => {
                                println!("[sync-error] addr={} error={}", addr, error);
                            }
                            (None, None) => {
                                println!("[sync-error] error={}", error);
                            }
                        }
                    }
                    None => {
                        sync_handle = None;
                        sync_progress_rx = None;
                    }
                }
            }
            maybe_progress = recv_sync_progress(&mut sync_progress_rx), if sync_progress_rx.is_some() => {
                if let Some(update) = maybe_progress {
                    if let TransferState::Progress { done_bytes, total_bytes, .. } = update.state {
                        println!(
                            "[file] {:?} transfer {} with {} progress: {}/{} bytes",
                            update.direction,
                            update.transfer_id,
                            update.peer_node_id,
                            done_bytes,
                            total_bytes
                        );
                    }
                } else {
                    sync_progress_rx = None;
                }
            }
        }
    }

    shutdown.store(true, Ordering::Relaxed);
    let _ = observer.join();

    if let Some(handle) = sync_handle {
        let _ = handle.shutdown_tx.send(());
    }

    if let Some(error) = watch_error {
        return Err(error);
    }

    Ok(())
}

async fn handle_send_file(
    path: PathBuf,
    wait_ms: u64,
    config_path: &Path,
) -> Result<(), NooboardError> {
    if !path.exists() {
        return Err(NooboardError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("file does not exist: {}", path.display()),
        )));
    }

    let sync_config = load_sync_config(config_path)?
        .ok_or_else(|| NooboardError::storage("sync is disabled in config".to_string()))?;
    let mut handle = start_sync_engine(sync_config)
        .await
        .map_err(|error| NooboardError::storage(error.to_string()))?;
    wait_sync_running(&mut handle.status_rx).await?;

    handle
        .file_tx
        .send(SendFileRequest {
            path: path.clone(),
            targets: None,
        })
        .await
        .map_err(|error| NooboardError::channel(format!("failed to queue file: {error}")))?;

    println!("queued file for sync: {}", path.display());
    tokio::time::sleep(Duration::from_millis(wait_ms.max(1))).await;

    let _ = handle.shutdown_tx.send(());
    Ok(())
}

async fn wait_sync_running(
    status_rx: &mut tokio::sync::watch::Receiver<SyncStatus>,
) -> Result<(), NooboardError> {
    if matches!(&*status_rx.borrow(), SyncStatus::Running) {
        return Ok(());
    }

    for _ in 0..20 {
        status_rx
            .changed()
            .await
            .map_err(|error| NooboardError::channel(error.to_string()))?;

        match &*status_rx.borrow() {
            SyncStatus::Running => return Ok(()),
            SyncStatus::Error(message) => {
                return Err(NooboardError::storage(format!(
                    "sync engine failed: {message}"
                )));
            }
            SyncStatus::Disabled => {
                return Err(NooboardError::storage(
                    "sync engine is disabled".to_string(),
                ));
            }
            _ => {}
        }
    }

    Err(NooboardError::storage(
        "sync engine did not reach running status".to_string(),
    ))
}

async fn recv_sync_event(sync_handle: &mut Option<SyncEngineHandle>) -> Option<SyncEvent> {
    match sync_handle {
        Some(handle) => handle.event_rx.recv().await,
        None => None,
    }
}

async fn recv_sync_progress(
    sync_progress_rx: &mut Option<tokio::sync::broadcast::Receiver<nooboard_sync::TransferUpdate>>,
) -> Option<nooboard_sync::TransferUpdate> {
    let Some(progress_rx) = sync_progress_rx.as_mut() else {
        return None;
    };

    loop {
        match progress_rx.recv().await {
            Ok(update) => return Some(update),
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
        }
    }
}

async fn prompt_file_decision(
    peer_node_id: &str,
    file_name: &str,
    file_size: u64,
) -> (bool, Option<String>) {
    let peer_node_id = peer_node_id.to_string();
    let file_name = file_name.to_string();

    let decision = tokio::task::spawn_blocking(move || {
        use std::io::{Write, stdin, stdout};

        println!(
            "[file] accept transfer from {}: {} ({} bytes)? [y/N]",
            peer_node_id, file_name, file_size
        );
        print!("> ");
        let _ = stdout().flush();

        let mut input = String::new();
        if stdin().read_line(&mut input).is_ok() {
            let normalized = input.trim().to_ascii_lowercase();
            if normalized == "y" || normalized == "yes" {
                return (true, None);
            }
        }

        (false, Some("rejected by user".to_string()))
    })
    .await;

    match decision {
        Ok(result) => result,
        Err(error) => (
            false,
            Some(format!("failed to read local decision: {error}")),
        ),
    }
}

fn persist_event(
    repository: &mut SqliteEventRepository,
    event: &ClipboardEvent,
) -> Result<(), NooboardError> {
    let created_at_ms = saturating_u128_to_i64(event.timestamp_millis());
    let applied_at_ms = current_timestamp_ms();

    repository
        .append_text(&event.text, None, None, created_at_ms, applied_at_ms)
        .map_err(storage_error_to_core)?;

    Ok(())
}

fn current_timestamp_ms() -> i64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);

    saturating_u128_to_i64(now)
}

fn saturating_u128_to_i64(value: u128) -> i64 {
    if value > i64::MAX as u128 {
        i64::MAX
    } else {
        value as i64
    }
}
