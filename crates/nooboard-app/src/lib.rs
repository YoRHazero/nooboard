pub mod error;
pub mod service;
pub mod status;

pub use error::AppError;
pub use nooboard_storage::ClipboardRecord;
pub use service::{AppService, AppServiceImpl, SyncStartConfig};
pub use status::{SyncState, SyncStatus};
