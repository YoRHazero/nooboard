use crate::{AppConfig, ConfigResult};

use super::paths::repo_root_path;
use super::spec::{BootstrapLaunch, BootstrapMode, ConfigTemplate};
use super::template::write_config_template;
use crate::ConfigError;

pub fn prepare_bootstrap_launch(launch: &BootstrapLaunch) -> ConfigResult<()> {
    match launch.mode {
        BootstrapMode::RepoDevelopment => ensure_repo_development_config(launch),
        BootstrapMode::ExplicitPath | BootstrapMode::UserDefault => {
            AppConfig::load(&launch.config_path)?;
            Ok(())
        }
    }
}

fn ensure_repo_development_config(launch: &BootstrapLaunch) -> ConfigResult<()> {
    let repo_root = repo_root_path()?;
    if !repo_root.exists() {
        return Err(ConfigError::InvalidBootstrap(format!(
            "repository root is unavailable for local development setup: {}",
            repo_root.display()
        )));
    }

    if !launch.config_path.exists() {
        write_config_template(&launch.config_path, ConfigTemplate::Development)?;
    }

    AppConfig::load(&launch.config_path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn repo_development_launch_creates_missing_config() -> ConfigResult<()> {
        let dir = tempdir()?;
        let config_path = dir.path().join(".dev-data").join("nooboard.toml");
        let launch = BootstrapLaunch {
            mode: BootstrapMode::RepoDevelopment,
            config_path: config_path.clone(),
        };

        prepare_bootstrap_launch(&launch)?;

        assert!(config_path.exists());
        let loaded = AppConfig::load(&config_path)?;
        assert_eq!(loaded.meta.profile, "dev");
        assert_eq!(loaded.identity.device_id, "nooboard-dev");
        assert_eq!(loaded.sync.auth.token, "token-for-sync");
        assert_eq!(
            loaded.identity.noob_id_file,
            config_path.parent().unwrap().join("noob_id")
        );
        assert_eq!(
            loaded.storage.db_root,
            config_path.parent().unwrap().join("data")
        );
        assert_eq!(
            loaded.sync.file.download_dir,
            config_path.parent().unwrap().join("downloads")
        );
        Ok(())
    }

    #[test]
    fn repo_development_launch_reports_invalid_existing_config() -> ConfigResult<()> {
        let dir = tempdir()?;
        let config_path = dir.path().join(".dev-data").join("nooboard.toml");
        std::fs::create_dir_all(config_path.parent().unwrap())?;
        std::fs::write(&config_path, "not = [valid")?;

        let launch = BootstrapLaunch {
            mode: BootstrapMode::RepoDevelopment,
            config_path,
        };

        let result = prepare_bootstrap_launch(&launch);
        assert!(result.is_err());
        Ok(())
    }
}
