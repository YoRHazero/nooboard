use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use clap::{Parser, Subcommand};
use nooboard_core::{ClipboardEvent, NooboardError};
use nooboard_platform::{ClipboardBackend, DEFAULT_WATCH_INTERVAL};
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "nooboard-cli",
    version,
    about = "Nooboard stage1 clipboard CLI"
)]
struct Cli {
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

    match cli.command {
        Commands::Get => handle_get(backend.as_ref()),
        Commands::Set { text } => handle_set(backend.as_ref(), text),
        Commands::Watch { interval_ms } => handle_watch(backend.as_ref(), interval_ms).await,
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
