use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::{ConfigError, ConfigResult};

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
}
