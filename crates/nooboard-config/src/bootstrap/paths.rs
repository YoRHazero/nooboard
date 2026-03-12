use std::path::PathBuf;

use crate::{ConfigError, ConfigResult};

pub const DEFAULT_CONFIG_FILE_NAME: &str = "nooboard.toml";

pub fn default_config_root() -> ConfigResult<PathBuf> {
    Ok(user_home_dir()?.join(".nooboard"))
}

pub fn default_config_path() -> ConfigResult<PathBuf> {
    Ok(default_config_root()?.join(DEFAULT_CONFIG_FILE_NAME))
}

pub fn repo_root_path() -> ConfigResult<PathBuf> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");

    std::fs::canonicalize(&repo_root).map_err(|error| {
        ConfigError::InvalidBootstrap(format!(
            "could not resolve repository root path {}: {error}",
            repo_root.display()
        ))
    })
}

pub fn repo_development_config_path() -> ConfigResult<PathBuf> {
    Ok(repo_root_path()?
        .join(".dev-data")
        .join(DEFAULT_CONFIG_FILE_NAME))
}

pub(crate) fn default_download_dir() -> ConfigResult<PathBuf> {
    Ok(user_home_dir()?.join("Downloads").join("nooboard"))
}

pub(crate) fn user_home_dir() -> ConfigResult<PathBuf> {
    #[cfg(windows)]
    {
        if let Some(path) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
            return Ok(path);
        }
        let home_drive = std::env::var_os("HOMEDRIVE");
        let home_path = std::env::var_os("HOMEPATH");
        if let (Some(drive), Some(path)) = (home_drive, home_path) {
            return Ok(PathBuf::from(format!(
                "{}{}",
                drive.to_string_lossy(),
                path.to_string_lossy()
            )));
        }
    }

    #[cfg(not(windows))]
    {
        if let Some(path) = std::env::var_os("HOME").map(PathBuf::from) {
            return Ok(path);
        }
    }

    Err(ConfigError::InvalidBootstrap(
        "could not resolve user home directory".to_string(),
    ))
}
