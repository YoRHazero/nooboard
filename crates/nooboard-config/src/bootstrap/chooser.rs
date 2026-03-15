use std::path::{Path, PathBuf};

use crate::{AppConfig, ConfigError, ConfigResult};

use super::launch::prepare_bootstrap_launch;
use super::paths::repo_development_config_path;
use super::probe::{inspect_existing_config, validate_existing_config_file_path};
use super::spec::{
    BootstrapLaunch, BootstrapMode, ConfigTemplate, CustomLocationProbe, ExistingConfigProbe,
};
use super::template::write_config_template;

pub fn prepare_existing_config_launch(path: &Path) -> ConfigResult<BootstrapLaunch> {
    match inspect_existing_config(path)? {
        ExistingConfigProbe::Valid { path } => Ok(BootstrapLaunch {
            mode: BootstrapMode::ExplicitPath,
            config_path: path,
        }),
        ExistingConfigProbe::Invalid { path, message } => {
            Err(ConfigError::InvalidBootstrap(format!(
                "existing config is invalid: {}\n{}",
                path.display(),
                message
            )))
        }
    }
}

pub fn rewrite_existing_config(path: &Path) -> ConfigResult<BootstrapLaunch> {
    validate_existing_config_file_path(path, "rewrite")?;
    match inspect_existing_config(path)? {
        ExistingConfigProbe::Valid { .. } => Err(ConfigError::InvalidBootstrap(format!(
            "rewrite requires an invalid config file: {}",
            path.display()
        ))),
        ExistingConfigProbe::Invalid { path, .. } => {
            write_config_template(&path, ConfigTemplate::Production)?;
            AppConfig::load(&path)?;
            Ok(BootstrapLaunch {
                mode: BootstrapMode::ExplicitPath,
                config_path: path,
            })
        }
    }
}

pub fn prepare_custom_location_launch(directory: &Path) -> ConfigResult<BootstrapLaunch> {
    let probe = super::probe::inspect_custom_location(directory)?;
    match probe {
        CustomLocationProbe::ReadyToCreate { config_path, .. } => {
            write_config_template(&config_path, ConfigTemplate::Production)?;
            AppConfig::load(&config_path)?;
            Ok(BootstrapLaunch {
                mode: BootstrapMode::ExplicitPath,
                config_path,
            })
        }
        CustomLocationProbe::ExistingValidConfig { config_path, .. } => Ok(BootstrapLaunch {
            mode: BootstrapMode::ExplicitPath,
            config_path,
        }),
        CustomLocationProbe::ExistingInvalidConfig {
            config_path,
            message,
            ..
        } => Err(ConfigError::InvalidBootstrap(format!(
            "custom location contains an invalid config: {}\n{}",
            config_path.display(),
            message
        ))),
    }
}

pub fn prepare_repo_development_launch() -> ConfigResult<BootstrapLaunch> {
    let config_path = repo_development_config_path()?;
    prepare_repo_development_launch_with_path(config_path)
}

fn prepare_repo_development_launch_with_path(
    config_path: PathBuf,
) -> ConfigResult<BootstrapLaunch> {
    let launch = BootstrapLaunch {
        mode: BootstrapMode::RepoDevelopment,
        config_path,
    };
    prepare_bootstrap_launch(&launch)?;
    Ok(launch)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;
    use crate::DEFAULT_CONFIG_FILE_NAME;

    #[test]
    fn prepare_existing_config_launch_accepts_valid_file() -> ConfigResult<()> {
        let dir = tempdir()?;
        let path = dir.path().join(DEFAULT_CONFIG_FILE_NAME);
        write_config_template(&path, ConfigTemplate::Production)?;

        let launch = prepare_existing_config_launch(&path)?;
        assert_eq!(launch.mode, BootstrapMode::ExplicitPath);
        assert_eq!(launch.config_path, path);
        Ok(())
    }

    #[test]
    fn rewrite_existing_config_rewrites_invalid_file() -> ConfigResult<()> {
        let dir = tempdir()?;
        let path = dir.path().join(DEFAULT_CONFIG_FILE_NAME);
        fs::write(&path, "not = [valid")?;

        let launch = rewrite_existing_config(&path)?;

        assert_eq!(launch.mode, BootstrapMode::ExplicitPath);
        let loaded = AppConfig::load(&launch.config_path)?;
        assert_eq!(loaded.meta.profile, "production");
        Ok(())
    }

    #[test]
    fn prepare_custom_location_launch_creates_missing_config() -> ConfigResult<()> {
        let dir = tempdir()?;

        let launch = prepare_custom_location_launch(dir.path())?;
        assert_eq!(launch.mode, BootstrapMode::ExplicitPath);
        assert!(launch.config_path.exists());
        Ok(())
    }

    #[test]
    fn prepare_custom_location_launch_reuses_existing_valid_config() -> ConfigResult<()> {
        let dir = tempdir()?;
        let path = dir.path().join(DEFAULT_CONFIG_FILE_NAME);
        write_config_template(&path, ConfigTemplate::Production)?;

        let launch = prepare_custom_location_launch(dir.path())?;
        assert_eq!(launch.config_path, path);
        Ok(())
    }

    #[test]
    fn prepare_repo_development_launch_creates_missing_dev_config() -> ConfigResult<()> {
        let dir = tempdir()?;
        let config_path = dir.path().join(".dev-data").join(DEFAULT_CONFIG_FILE_NAME);

        let launch = prepare_repo_development_launch_with_path(config_path.clone())?;

        assert_eq!(launch.mode, BootstrapMode::RepoDevelopment);
        assert!(config_path.exists());
        Ok(())
    }
}
