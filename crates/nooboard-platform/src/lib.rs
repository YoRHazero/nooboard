pub mod backend;
pub mod error;
pub mod model;

pub use backend::{ClipboardBackend, ClipboardEventSender, DEFAULT_WATCH_INTERVAL};
pub use error::NooboardError;
pub use model::ClipboardEvent;
