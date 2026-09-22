//! Native clipboard access with an explicitly owned service and shareable async handles.
//! Snapshots describe the latest observation; intermediate changes may coalesce.
mod api;
mod backend;
mod error;
mod formats;
mod model;
mod options;
mod runtime;

pub use api::{Clipboard, ClipboardService};
pub use error::{Error, Result};
pub use formats::image::{ImageData, ImageEncoding, MAX_IMAGE_BYTES};
pub use model::{Origin, Payload, ReadState, ServiceStatus, SkipReason, Snapshot};
pub use options::{BackendPreference, Limits, Options};
