use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use clap::{Parser, Subcommand};
use nooboard_core::{ClipboardEvent, NooboardError};
use nooboard_platform::{ClipboardBackend, DEFAULT_WATCH_INTERVAL};
use nooboard_storage::{SqliteEventRepository, StorageError, default_dev_config_path};
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "nooboard-cli",
    version,
    about = "Nooboard stage2 clipboard CLI"
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
            handle_watch(backend.as_ref(), interval_ms, &config_path).await
        }
        Commands::History { limit, keyword } => handle_history(limit, keyword, &config_path),
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
            .list_history(limit)
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
) -> Result<(), NooboardError> {
    let mut repository = open_repository(config_path)?;

    let (sender, mut receiver) = mpsc::channel::<ClipboardEvent>(64);
    let shutdown = Arc::new(AtomicBool::new(false));
    let interval = Duration::from_millis(interval_ms.max(1));

    let observer = backend.watch_changes(sender, shutdown.clone(), interval)?;

    println!(
        "watching clipboard changes (interval={}ms). Press Ctrl+C to stop.",
        interval_ms.max(1)
    );

    let mut watch_error: Option<NooboardError> = None;

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
                        println!("[{}] {}", event.timestamp_millis(), event.text);

                        if let Err(error) = persist_event(&mut repository, &event) {
                            shutdown.store(true, Ordering::Relaxed);
                            watch_error = Some(error);
                            break;
                        }
                    }
                    None => {
                        shutdown.store(true, Ordering::Relaxed);
                        break;
                    }
                }
            }
        }
    }

    shutdown.store(true, Ordering::Relaxed);
    let _ = observer.join();

    if let Some(error) = watch_error {
        return Err(error);
    }

    Ok(())
}

fn persist_event(
    repository: &mut SqliteEventRepository,
    event: &ClipboardEvent,
) -> Result<(), NooboardError> {
    let created_at_ms = saturating_u128_to_i64(event.timestamp_millis());
    let applied_at_ms = current_timestamp_ms();

    repository
        .append_local_text(&event.text, created_at_ms, applied_at_ms)
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
