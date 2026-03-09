use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::StorageError;

pub const STORAGE_SCHEMA_VERSION: &str = "v0.2.0";

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub storage: StorageConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfig {
    pub db_root: PathBuf,
    #[serde(default)]
    pub retain_old_versions: usize,
    #[serde(default)]
    pub lifecycle: LifecycleConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LifecycleConfig {
    #[serde(default = "default_history_window_days")]
    pub history_window_days: u32,
    #[serde(default = "default_dedup_window_days")]
    pub dedup_window_days: u32,
    #[serde(default = "default_gc_every_inserts")]
    pub gc_every_inserts: u32,
    #[serde(default = "default_gc_batch_size")]
    pub gc_batch_size: u32,
}

impl Default for LifecycleConfig {
    fn default() -> Self {
        Self {
            history_window_days: default_history_window_days(),
            dedup_window_days: default_dedup_window_days(),
            gc_every_inserts: default_gc_every_inserts(),
            gc_batch_size: default_gc_batch_size(),
        }
    }
}

impl AppConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let path = path.as_ref();
        let raw = fs::read_to_string(path)?;
        let mut config: AppConfig =
            toml::from_str(&raw).map_err(|source| StorageError::ConfigParse {
                path: path.to_path_buf(),
                source,
            })?;

        let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
        config.storage.resolve_relative_paths(base_dir);
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), StorageError> {
        self.storage.validate()
    }
}

impl StorageConfig {
    pub fn current_version_dir(&self) -> PathBuf {
        self.db_root.join(STORAGE_SCHEMA_VERSION)
    }

    pub fn db_path(&self) -> PathBuf {
        self.current_version_dir().join("nooboard.db")
    }

    fn resolve_relative_paths(&mut self, base_dir: &Path) {
        absolutize_if_relative(&mut self.db_root, base_dir);
    }

    pub(crate) fn validate(&self) -> Result<(), StorageError> {
        if self.lifecycle.history_window_days < 1 {
            return Err(StorageError::InvalidConfig(
                "storage.lifecycle.history_window_days must be >= 1".to_string(),
            ));
        }

        if self.lifecycle.dedup_window_days < self.lifecycle.history_window_days {
            return Err(StorageError::InvalidConfig(
                "storage.lifecycle.dedup_window_days must be >= history_window_days".to_string(),
            ));
        }

        if self.lifecycle.gc_every_inserts < 1 {
            return Err(StorageError::InvalidConfig(
                "storage.lifecycle.gc_every_inserts must be >= 1".to_string(),
            ));
        }

        if self.lifecycle.gc_batch_size < 1 {
            return Err(StorageError::InvalidConfig(
                "storage.lifecycle.gc_batch_size must be >= 1".to_string(),
            ));
        }

        Ok(())
    }
}

pub fn default_dev_config_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("configs")
        .join("dev.toml")
}

const fn default_history_window_days() -> u32 {
    7
}

const fn default_dedup_window_days() -> u32 {
    14
}

const fn default_gc_every_inserts() -> u32 {
    200
}

const fn default_gc_batch_size() -> u32 {
    500
}

fn absolutize_if_relative(path: &mut PathBuf, base_dir: &Path) {
    if path.is_relative() {
        *path = base_dir.join(&*path);
    }
}

#[cfg(test)]
mod tests {
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "nooboard-storage-config-{name}-{}-{millis}",
            process::id()
        ))
    }

    #[test]
    fn load_rejects_invalid_lifecycle_window() -> Result<(), StorageError> {
        let dir = temp_dir("invalid-lifecycle");
        fs::create_dir_all(&dir)?;
        let config_path = dir.join("dev.toml");

        let raw = r#"
[storage]
db_root = "./data"
retain_old_versions = 0

[storage.lifecycle]
history_window_days = 7
dedup_window_days = 6
gc_every_inserts = 1
gc_batch_size = 1
"#;

        fs::write(&config_path, raw)?;
        let result = AppConfig::load(&config_path);

        assert!(matches!(result, Err(StorageError::InvalidConfig(_))));

        let _ = fs::remove_dir_all(dir);
        Ok(())
    }

    #[test]
    fn load_resolves_relative_paths_from_config_parent() -> Result<(), StorageError> {
        let dir = temp_dir("relative-paths");
        fs::create_dir_all(&dir)?;
        let config_path = dir.join("dev.toml");

        let raw = r#"
[storage]
db_root = "./data"
retain_old_versions = 0

[storage.lifecycle]
history_window_days = 7
dedup_window_days = 14
gc_every_inserts = 1
gc_batch_size = 1
"#;

        fs::write(&config_path, raw)?;
        let config = AppConfig::load(&config_path)?;

        assert_eq!(config.storage.db_root, dir.join("./data"));
        assert_eq!(
            config.storage.current_version_dir(),
            dir.join(format!("./data/{STORAGE_SCHEMA_VERSION}"))
        );

        let _ = fs::remove_dir_all(dir);
        Ok(())
    }

    #[test]
    fn load_ignores_legacy_schema_paths_fields() -> Result<(), StorageError> {
        let dir = temp_dir("legacy-schema-fields");
        fs::create_dir_all(&dir)?;
        let config_path = dir.join("dev.toml");

        let raw = r#"
[storage]
db_root = "./data"
retain_old_versions = 0
schema_version = "v9.9.9"
schema_sql = "./old/schema.sql"
queries_dir = "./old/queries"

[storage.lifecycle]
history_window_days = 7
dedup_window_days = 14
gc_every_inserts = 1
gc_batch_size = 1
"#;

        fs::write(&config_path, raw)?;
        let config = AppConfig::load(&config_path)?;

        assert_eq!(
            config.storage.current_version_dir(),
            dir.join(format!("./data/{STORAGE_SCHEMA_VERSION}"))
        );

        let _ = fs::remove_dir_all(dir);
        Ok(())
    }
}
