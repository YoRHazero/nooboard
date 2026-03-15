use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::{ConfigError, ConfigResult};

use super::DEFAULT_CONFIG_FILE_NAME;
use super::paths::repo_development_config_path;
use super::spec::{CustomLocationProbe, ExistingConfigProbe, RepoDevelopmentProbe};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultConfigState {
    Missing,
    Compatible,
    Incompatible { found_version: Option<u32> },
}

#[derive(Debug, Deserialize, Default)]
struct VersionProbe {
    #[serde(default)]
    meta: MetaProbe,
}

#[derive(Debug, Deserialize, Default)]
struct MetaProbe {
    config_version: Option<u32>,
}

pub fn inspect_default_config_state(
    path: &Path,
    expected_version: u32,
) -> ConfigResult<DefaultConfigState> {
    if !path.exists() {
        return Ok(DefaultConfigState::Missing);
    }

    if !path.is_file() {
        return Err(ConfigError::InvalidBootstrap(format!(
            "default config path is not a file: {}",
            path.display()
        )));
    }

    let raw = fs::read_to_string(path)?;
    let probe: VersionProbe = toml::from_str(&raw).map_err(|source| ConfigError::Parse {
        path: path.to_path_buf(),
        source,
    })?;

    match probe.meta.config_version {
        Some(found_version) if found_version == expected_version => {
            Ok(DefaultConfigState::Compatible)
        }
        found_version => Ok(DefaultConfigState::Incompatible { found_version }),
    }
}

pub fn inspect_existing_config(path: &Path) -> ConfigResult<ExistingConfigProbe> {
    let path = path.to_path_buf();
    match validate_existing_config_file_path(&path, "existing config") {
        Ok(()) => match crate::AppConfig::load(&path) {
            Ok(_) => Ok(ExistingConfigProbe::Valid { path }),
            Err(error) => Ok(ExistingConfigProbe::Invalid {
                path,
                message: error.to_string(),
            }),
        },
        Err(error) => Ok(ExistingConfigProbe::Invalid {
            path,
            message: error.to_string(),
        }),
    }
}

pub fn inspect_custom_location(directory: &Path) -> ConfigResult<CustomLocationProbe> {
    validate_directory_path(directory, "custom location")?;
    let directory = directory.to_path_buf();
    let config_path = directory.join(DEFAULT_CONFIG_FILE_NAME);

    if !config_path.exists() {
        return Ok(CustomLocationProbe::ReadyToCreate {
            directory,
            config_path,
        });
    }

    match crate::AppConfig::load(&config_path) {
        Ok(_) => Ok(CustomLocationProbe::ExistingValidConfig {
            directory,
            config_path,
        }),
        Err(error) => Ok(CustomLocationProbe::ExistingInvalidConfig {
            directory,
            config_path,
            message: error.to_string(),
        }),
    }
}

pub fn inspect_repo_development() -> ConfigResult<RepoDevelopmentProbe> {
    match repo_development_config_path() {
        Ok(config_path) => inspect_repo_development_with_path(config_path),
        Err(error) => Ok(RepoDevelopmentProbe::Unavailable {
            message: error.to_string(),
        }),
    }
}

pub(crate) fn validate_existing_config_file_path(path: &Path, source: &str) -> ConfigResult<()> {
    if !path.exists() {
        return Err(ConfigError::InvalidBootstrap(format!(
            "{source} path does not exist: {}",
            path.display()
        )));
    }

    if !path.is_file() {
        return Err(ConfigError::InvalidBootstrap(format!(
            "{source} path is not a file: {}",
            path.display()
        )));
    }

    Ok(())
}

fn validate_directory_path(path: &Path, source: &str) -> ConfigResult<()> {
    if !path.exists() {
        return Err(ConfigError::InvalidBootstrap(format!(
            "{source} path does not exist: {}",
            path.display()
        )));
    }

    if !path.is_dir() {
        return Err(ConfigError::InvalidBootstrap(format!(
            "{source} path is not a directory: {}",
            path.display()
        )));
    }

    Ok(())
}

fn inspect_repo_development_with_path(config_path: PathBuf) -> ConfigResult<RepoDevelopmentProbe> {
    if !config_path.exists() {
        return Ok(RepoDevelopmentProbe::Available { config_path });
    }

    match crate::AppConfig::load(&config_path) {
        Ok(_) => Ok(RepoDevelopmentProbe::Available { config_path }),
        Err(error) => Ok(RepoDevelopmentProbe::Unavailable {
            message: error.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn missing_default_config_is_reported_as_missing() -> ConfigResult<()> {
        let dir = tempdir()?;
        let path = dir.path().join("nooboard.toml");
        assert_eq!(
            inspect_default_config_state(&path, 3)?,
            DefaultConfigState::Missing
        );
        Ok(())
    }

    #[test]
    fn matching_version_is_compatible() -> ConfigResult<()> {
        let dir = tempdir()?;
        let path = dir.path().join("nooboard.toml");
        fs::write(&path, "[meta]\nconfig_version = 3\n")?;

        assert_eq!(
            inspect_default_config_state(&path, 3)?,
            DefaultConfigState::Compatible
        );
        Ok(())
    }

    #[test]
    fn mismatched_version_is_incompatible() -> ConfigResult<()> {
        let dir = tempdir()?;
        let path = dir.path().join("nooboard.toml");
        fs::write(&path, "[meta]\nconfig_version = 2\n")?;

        assert_eq!(
            inspect_default_config_state(&path, 3)?,
            DefaultConfigState::Incompatible {
                found_version: Some(2),
            }
        );
        Ok(())
    }

    #[test]
    fn missing_version_is_incompatible() -> ConfigResult<()> {
        let dir = tempdir()?;
        let path = dir.path().join("nooboard.toml");
        fs::write(&path, "profile = \"legacy\"\n")?;

        assert_eq!(
            inspect_default_config_state(&path, 3)?,
            DefaultConfigState::Incompatible {
                found_version: None,
            }
        );
        Ok(())
    }

    #[test]
    fn inspect_existing_config_reports_valid_and_invalid() -> ConfigResult<()> {
        let dir = tempdir()?;
        let valid = dir.path().join("valid.toml");
        let invalid = dir.path().join("invalid.toml");
        super::super::template::write_config_template(
            &valid,
            super::super::spec::ConfigTemplate::Production,
        )?;
        fs::write(&invalid, "not = [valid")?;

        assert!(matches!(
            inspect_existing_config(&valid)?,
            ExistingConfigProbe::Valid { .. }
        ));
        assert!(matches!(
            inspect_existing_config(&invalid)?,
            ExistingConfigProbe::Invalid { .. }
        ));
        Ok(())
    }

    #[test]
    fn inspect_custom_location_reports_create_and_invalid_states() -> ConfigResult<()> {
        let dir = tempdir()?;
        assert!(matches!(
            inspect_custom_location(dir.path())?,
            CustomLocationProbe::ReadyToCreate { .. }
        ));

        let invalid = dir.path().join(DEFAULT_CONFIG_FILE_NAME);
        fs::write(&invalid, "not = [valid")?;
        assert!(matches!(
            inspect_custom_location(dir.path())?,
            CustomLocationProbe::ExistingInvalidConfig { .. }
        ));
        Ok(())
    }

    #[test]
    fn inspect_repo_development_reports_invalid_existing_config_as_unavailable() -> ConfigResult<()>
    {
        let dir = tempdir()?;
        let config_path = dir.path().join(".dev-data").join(DEFAULT_CONFIG_FILE_NAME);
        fs::create_dir_all(config_path.parent().expect("parent"))?;
        fs::write(&config_path, "not = [valid")?;

        assert!(matches!(
            inspect_repo_development_with_path(config_path)?,
            RepoDevelopmentProbe::Unavailable { .. }
        ));
        Ok(())
    }
}
