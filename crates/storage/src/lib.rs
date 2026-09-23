#![doc = include_str!("../README.md")]
mod api;
mod backend;
mod error;
mod model;
mod options;
mod runtime;

pub use api::{History, Settings, Storage, StorageService};
pub use error::{Error, ErrorKind, Result};
pub use model::*;
pub use options::{BackendConfig, Options};
#[cfg(feature = "sqlite")]
pub use options::{SqliteLocation, SqliteOptions};
