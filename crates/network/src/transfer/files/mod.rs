//! Bounded disk staging and publication. No network or clipboard dependencies.
mod filesystem;
mod prepare;
mod receive;
#[cfg(test)]
mod tests;

use crate::error::{Failure as Error, InternalResult as Result};
pub use prepare::PreparedBatch;
pub use receive::IncomingBatch;
use std::sync::atomic::{AtomicBool, Ordering};

pub const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub const MAX_BATCH_BYTES: u64 = 8 * 1024 * 1024 * 1024;
pub const MAX_FILES: usize = 256;
pub const CHUNK_BYTES: usize = 256 * 1024;

#[derive(Clone, Debug)]
pub struct FileSpec {
    pub name: String,
    pub bytes: u64,
    pub sha256: [u8; 32],
}

pub fn check_cancelled(cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Acquire) {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}
