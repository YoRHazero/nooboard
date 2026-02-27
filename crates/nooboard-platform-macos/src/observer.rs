use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use nooboard_platform::{ClipboardEvent, ClipboardEventSender, NooboardError};

use crate::pasteboard::{pasteboard_change_count, read_text_from_pasteboard};

pub(crate) fn spawn_observer(
    sender: ClipboardEventSender,
    shutdown: Arc<AtomicBool>,
    interval: Duration,
) -> Result<JoinHandle<()>, NooboardError> {
    let mut last_change_count = pasteboard_change_count()?;

    let handle = thread::spawn(move || {
        while !shutdown.load(Ordering::Relaxed) {
            match pasteboard_change_count() {
                Ok(current_change_count) if current_change_count != last_change_count => {
                    last_change_count = current_change_count;

                    if let Ok(Some(text)) = read_text_from_pasteboard() {
                        let event = ClipboardEvent::new(text);
                        if sender.blocking_send(event).is_err() {
                            break;
                        }
                    }
                }
                Ok(_) => {}
                Err(error) => {
                    eprintln!("watch error: {error}");
                }
            }

            thread::sleep(interval);
        }
    });

    Ok(handle)
}
