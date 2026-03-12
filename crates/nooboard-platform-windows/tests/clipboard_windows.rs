#![cfg(target_os = "windows")]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use nooboard_platform::ClipboardBackend;
use nooboard_platform_windows::WindowsClipboardBackend;
use tokio::sync::mpsc;
use tokio::time::{Instant, sleep, timeout};

const OBSERVER_INTERVAL: Duration = Duration::from_millis(25);
const WAIT_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(25);

struct ClipboardTextSnapshot {
    previous_text: Option<String>,
}

impl ClipboardTextSnapshot {
    fn capture(backend: &WindowsClipboardBackend) -> Self {
        Self {
            previous_text: backend.read_text().ok().flatten(),
        }
    }
}

impl Drop for ClipboardTextSnapshot {
    fn drop(&mut self) {
        if let Some(text) = &self.previous_text {
            let backend = WindowsClipboardBackend::new();
            let _ = backend.write_text(text);
        }
    }
}

fn unique_text(prefix: &str) -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after unix epoch")
        .as_nanos();
    format!("nooboard-{prefix}-{}-{timestamp}", std::process::id())
}

async fn wait_for_clipboard_text(
    backend: &WindowsClipboardBackend,
    expected: &str,
) -> Result<(), String> {
    let deadline = Instant::now() + WAIT_TIMEOUT;

    loop {
        match backend.read_text() {
            Ok(Some(text)) if text == expected => return Ok(()),
            Ok(Some(_)) | Ok(None) | Err(_) => {}
        }

        if Instant::now() >= deadline {
            break;
        }

        sleep(POLL_INTERVAL).await;
    }

    Err(format!(
        "timed out waiting for clipboard text to become {expected:?}"
    ))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn read_and_write_text_round_trip_with_real_windows_clipboard() {
    let backend = WindowsClipboardBackend::new();
    let _snapshot = ClipboardTextSnapshot::capture(&backend);
    let expected = unique_text("roundtrip");

    backend
        .write_text(&expected)
        .expect("write_text must succeed");
    wait_for_clipboard_text(&backend, &expected)
        .await
        .expect("clipboard must eventually expose the written text");

    let actual = backend
        .read_text()
        .expect("read_text must succeed")
        .expect("clipboard must contain text");
    assert_eq!(actual, expected);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn watch_changes_emits_real_windows_clipboard_update() {
    let observer_backend = WindowsClipboardBackend::new();
    let writer_backend = WindowsClipboardBackend::new();
    let _snapshot = ClipboardTextSnapshot::capture(&observer_backend);

    let baseline = unique_text("baseline");
    writer_backend
        .write_text(&baseline)
        .expect("baseline write must succeed");
    wait_for_clipboard_text(&observer_backend, &baseline)
        .await
        .expect("baseline clipboard write must be observable");

    let (sender, mut receiver) = mpsc::channel(8);
    let shutdown = Arc::new(AtomicBool::new(false));
    let worker = observer_backend
        .watch_changes(sender, Arc::clone(&shutdown), OBSERVER_INTERVAL)
        .expect("watch_changes must start");

    sleep(OBSERVER_INTERVAL * 2).await;

    let expected = unique_text("watch");
    writer_backend
        .write_text(&expected)
        .expect("updated write must succeed");

    let event = timeout(WAIT_TIMEOUT, async {
        loop {
            let event = receiver
                .recv()
                .await
                .expect("watch worker must keep sender alive while running");
            if event.text == expected {
                return event;
            }
        }
    })
    .await
    .expect("watch_changes must emit the updated clipboard text");

    assert_eq!(event.text, expected);

    shutdown.store(true, Ordering::Relaxed);
    worker.join().expect("watch worker must shut down cleanly");
}
