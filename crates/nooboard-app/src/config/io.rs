use std::fs;
use std::path::Path;

use super::noob_id::{
    absolutize_if_relative, regenerate_noob_id as regenerate_noob_id_file, resolve_or_init_noob_id,
};
use super::schema::AppConfig;
use crate::{AppError, AppResult};

impl AppConfig {
    pub fn load(path: impl AsRef<Path>) -> AppResult<Self> {
        let path = path.as_ref();
        let raw = fs::read_to_string(path)?;
        let mut config: Self = toml::from_str(&raw).map_err(|source| AppError::ConfigParse {
            path: path.to_path_buf(),
            source,
        })?;

        let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
        config.resolve_relative_paths(base_dir);
        config.ensure_noob_id_loaded()?;
        config.validate()?;
        Ok(config)
    }

    pub fn save_atomically(&self, path: impl AsRef<Path>) -> AppResult<()> {
        let path = path.as_ref();
        let parent = path.parent().ok_or_else(|| {
            AppError::InvalidConfig(format!("config path `{}` has no parent", path.display()))
        })?;
        fs::create_dir_all(parent)?;

        let serialized = toml::to_string_pretty(self)?;
        let temp_path = parent.join(format!(
            ".{}.tmp-{}",
            path.file_name()
                .and_then(|v| v.to_str())
                .unwrap_or("config"),
            std::process::id()
        ));

        fs::write(&temp_path, serialized)?;
        fs::rename(&temp_path, path)?;
        Ok(())
    }

    pub fn regenerate_noob_id(config_path: impl AsRef<Path>) -> AppResult<String> {
        let config_path = config_path.as_ref();
        let raw = fs::read_to_string(config_path)?;
        let mut config: Self = toml::from_str(&raw).map_err(|source| AppError::ConfigParse {
            path: config_path.to_path_buf(),
            source,
        })?;

        let base_dir = config_path.parent().unwrap_or_else(|| Path::new("."));
        config.resolve_relative_paths(base_dir);
        let generated = regenerate_noob_id_file(&config.identity.noob_id_file)?;
        Ok(generated)
    }

    fn resolve_relative_paths(&mut self, base_dir: &Path) {
        absolutize_if_relative(&mut self.identity.noob_id_file, base_dir);
        absolutize_if_relative(&mut self.storage.db_root, base_dir);
        absolutize_if_relative(&mut self.sync.file.download_dir, base_dir);
    }

    fn ensure_noob_id_loaded(&mut self) -> AppResult<()> {
        let noob_id = resolve_or_init_noob_id(&self.identity.noob_id_file)?;
        self.noob_id = Some(noob_id);
        Ok(())
    }
}
