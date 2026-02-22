use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::StorageError;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub storage: StorageSection,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StorageSection {
    pub db_path: PathBuf,
    pub schema_path: PathBuf,
}

impl AppConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let path = path.as_ref();
        let raw = fs::read_to_string(path)?;
        toml::from_str(&raw).map_err(|source| StorageError::ConfigParse {
            path: path.to_path_buf(),
            source,
        })
    }
}

pub fn default_dev_config_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("configs")
        .join("dev.toml")
}
