pub(crate) mod contract;
#[cfg(feature = "sqlite")]
mod sqlite;
use crate::{BackendConfig, Result};
use contract::Backend;

pub(crate) async fn open(config: BackendConfig) -> Result<Box<dyn Backend>> {
    match config {
        #[cfg(feature = "sqlite")]
        BackendConfig::Sqlite(options) => Ok(Box::new(sqlite::Sqlite::open(options).await?)),
    }
}
