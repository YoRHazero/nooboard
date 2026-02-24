pub mod auth;
pub mod config;
pub mod connection;
pub mod discovery;
pub mod engine;
pub mod error;
pub mod protocol;
pub mod transport;

pub use config::SyncConfig;
pub use engine::{
    FileDecisionInput, SyncControlCommand, SyncEngineHandle, SyncEvent, SyncStatus,
    start_sync_engine,
};
pub use error::SyncError;
