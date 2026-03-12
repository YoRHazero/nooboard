use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use nooboard_platform::{ClipboardEvent, ClipboardEventSender, NooboardError};

use crate::clipboard::{clipboard_sequence_number, read_text_from_clipboard};

pub(crate) fn spawn_observer(
    sender: ClipboardEventSender,
    shutdown: Arc<AtomicBool>,
    interval: Duration,
) -> Result<JoinHandle<()>, NooboardError> {
    let mut last_sequence = clipboard_sequence_number();

    let handle = thread::spawn(move || {
        while !shutdown.load(Ordering::Relaxed) {
            let current_sequence = clipboard_sequence_number();
            if current_sequence != last_sequence {
                last_sequence = current_sequence;

                match read_text_from_clipboard() {
                    Ok(Some(text)) => {
                        let event = ClipboardEvent::new(text);
                        if sender.blocking_send(event).is_err() {
                            break;
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        eprintln!("watch error: {error}");
                    }
                }
            }

            thread::sleep(interval);
        }
    });

    Ok(handle)
}
