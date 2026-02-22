use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use clap::{Parser, Subcommand};
use nooboard_core::{ClipboardEvent, NooboardError};
use nooboard_platform::{ClipboardBackend, DEFAULT_WATCH_INTERVAL};
use nooboard_storage::{ClipboardRepository, SqliteClipboardRepository, default_dev_config_path};
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "nooboard-cli",
    version,
    about = "Nooboard stage1 clipboard CLI"
)]
struct Cli {
    /// Config file path (defaults to configs/dev.toml)
    #[arg(long, global = true)]
    config: Option<String>,

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
    /// Show recent clipboard history
    History {
        /// Maximum number of records to return
        #[arg(long, default_value_t = 20)]
        limit: usize,
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
    let backend = create_backend()?;
    let config_path = cli.config.as_deref().map(Path::new);

    match cli.command {
        Commands::Get => handle_get(backend.as_ref()),
        Commands::Set { text } => handle_set(backend.as_ref(), text),
        Commands::Watch { interval_ms } => {
            let repository = create_repository(config_path)?;
            handle_watch(backend.as_ref(), &repository, interval_ms).await
        }
        Commands::History { limit } => {
            let repository = create_repository(config_path)?;
            handle_history(&repository, limit)
        }
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

fn create_repository(
    config_path: Option<&Path>,
) -> Result<SqliteClipboardRepository, NooboardError> {
    let default_config_path;
    let config_path = match config_path {
        Some(path) => path,
        None => {
            default_config_path = default_dev_config_path();
            default_config_path.as_path()
        }
    };
    let repository =
        SqliteClipboardRepository::open_from_config(config_path).map_err(map_storage_error)?;
    repository.init_schema().map_err(map_storage_error)?;
    Ok(repository)
}

fn map_storage_error(error: nooboard_storage::StorageError) -> NooboardError {
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

async fn handle_watch(
    backend: &dyn ClipboardBackend,
    repository: &dyn ClipboardRepository,
    interval_ms: u64,
) -> Result<(), NooboardError> {
    let (sender, mut receiver) = mpsc::channel::<ClipboardEvent>(64);
    let shutdown = Arc::new(AtomicBool::new(false));
    let interval = Duration::from_millis(interval_ms.max(1));

    let observer = backend.watch_changes(sender, shutdown.clone(), interval)?;

    println!(
        "watching clipboard changes (interval={}ms). Press Ctrl+C to stop.",
        interval_ms.max(1)
    );

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
                        let captured_at = i64::try_from(event.timestamp_millis()).map_err(|_| {
                            NooboardError::storage("clipboard event timestamp overflowed i64")
                        })?;
                        repository
                            .insert_text_event(&event.text, captured_at)
                            .map_err(map_storage_error)?;
                        println!("[{}] {}", event.timestamp_millis(), event.text);
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

    Ok(())
}

fn handle_history(repository: &dyn ClipboardRepository, limit: usize) -> Result<(), NooboardError> {
    let records = repository.list_recent(limit).map_err(map_storage_error)?;

    if records.is_empty() {
        println!("no clipboard history records");
        return Ok(());
    }

    for record in records {
        let single_line_content = record.content.replace('\n', "\\n");
        println!("[{}] {}", record.captured_at, single_line_content);
    }

    Ok(())
}
