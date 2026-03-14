use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{ConfigError, ConfigResult};

use super::DEFAULT_CONFIG_FILE_NAME;
use super::spec::{
    BootstrapChooserContext, BootstrapChooserReason, BootstrapLaunch, BootstrapMode, ConfigTemplate,
};
use super::template::write_config_template;

const DEFAULT_MANAGED_ENTRY_NAMES: [&str; 3] = [DEFAULT_CONFIG_FILE_NAME, "noob_id", "data"];

pub fn prepare_default_config_from_chooser(
    context: &BootstrapChooserContext,
) -> ConfigResult<BootstrapLaunch> {
    match context.reason {
        BootstrapChooserReason::MissingDefaultConfig => {}
        BootstrapChooserReason::DefaultConfigVersionMismatch { found_version, .. } => {
            archive_incompatible_default_bundle(&context.default_config_path, found_version)?;
        }
        BootstrapChooserReason::ExplicitChooserRequest => {
            return Err(ConfigError::InvalidBootstrap(
                "explicit chooser request cannot recreate the default config without caller confirmation"
                    .to_string(),
            ));
        }
    }

    write_config_template(&context.default_config_path, ConfigTemplate::Production)?;
    Ok(BootstrapLaunch {
        mode: BootstrapMode::UserDefault,
        config_path: context.default_config_path.clone(),
    })
}

fn archive_incompatible_default_bundle(
    default_config_path: &Path,
    found_version: Option<u32>,
) -> ConfigResult<PathBuf> {
    let suffix = archive_timestamp_suffix()?;
    archive_incompatible_default_bundle_with_suffix(default_config_path, found_version, &suffix)
}

fn archive_incompatible_default_bundle_with_suffix(
    default_config_path: &Path,
    found_version: Option<u32>,
    suffix: &str,
) -> ConfigResult<PathBuf> {
    let default_root = default_config_path.parent().ok_or_else(|| {
        ConfigError::InvalidBootstrap(format!(
            "default config path has no parent: {}",
            default_config_path.display()
        ))
    })?;

    let archive_dir = default_root.join("archive").join(format!(
        "{}_{}",
        archive_version_label(found_version),
        suffix
    ));
    fs::create_dir_all(&archive_dir)?;

    for entry_name in DEFAULT_MANAGED_ENTRY_NAMES {
        let source = default_root.join(entry_name);
        if !source.exists() {
            continue;
        }

        fs::rename(&source, archive_dir.join(entry_name))?;
    }

    Ok(archive_dir)
}

fn archive_timestamp_suffix() -> ConfigResult<String> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| ConfigError::InvalidBootstrap(format!("system clock error: {error}")))?;
    Ok(duration.as_millis().to_string())
}

fn archive_version_label(found_version: Option<u32>) -> String {
    match found_version {
        Some(version) => format!("v{version}"),
        None => "vunknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;
    use crate::{APP_CONFIG_VERSION, AppConfig};

    #[test]
    fn missing_default_config_creates_new_template_without_archive() -> ConfigResult<()> {
        let dir = tempdir()?;
        let config_path = dir.path().join(DEFAULT_CONFIG_FILE_NAME);
        let context = BootstrapChooserContext {
            default_config_path: config_path.clone(),
            reason: BootstrapChooserReason::MissingDefaultConfig,
        };

        let launch = prepare_default_config_from_chooser(&context)?;
        assert_eq!(launch.config_path, config_path);
        assert_eq!(launch.mode, BootstrapMode::UserDefault);
        assert!(!dir.path().join("archive").exists());

        let config = AppConfig::load(&launch.config_path)?;
        assert_eq!(config.meta.config_version, APP_CONFIG_VERSION);
        Ok(())
    }

    #[test]
    fn explicit_chooser_request_cannot_reset_default_config() -> ConfigResult<()> {
        let dir = tempdir()?;
        let config_path = dir.path().join(DEFAULT_CONFIG_FILE_NAME);
        let context = BootstrapChooserContext {
            default_config_path: config_path,
            reason: BootstrapChooserReason::ExplicitChooserRequest,
        };

        let result = prepare_default_config_from_chooser(&context);
        assert!(matches!(result, Err(ConfigError::InvalidBootstrap(_))));
        Ok(())
    }

    #[test]
    fn incompatible_default_bundle_is_archived_before_reset() -> ConfigResult<()> {
        let dir = tempdir()?;
        let config_path = dir.path().join(DEFAULT_CONFIG_FILE_NAME);
        fs::write(&config_path, "[meta]\nconfig_version = 2\n")?;
        fs::write(dir.path().join("noob_id"), "legacy-noob-id\n")?;
        fs::create_dir_all(dir.path().join("data"))?;
        fs::write(dir.path().join("data").join("db.sqlite"), "legacy")?;

        let archive_dir =
            archive_incompatible_default_bundle_with_suffix(&config_path, Some(2), "123456")?;

        assert_eq!(archive_dir, dir.path().join("archive").join("v2_123456"));
        assert!(archive_dir.join(DEFAULT_CONFIG_FILE_NAME).exists());
        assert!(archive_dir.join("noob_id").exists());
        assert!(archive_dir.join("data").exists());
        assert!(!config_path.exists());
        assert!(!dir.path().join("noob_id").exists());
        assert!(!dir.path().join("data").exists());
        Ok(())
    }

    #[test]
    fn reset_archives_managed_bundle_and_rewrites_default_config() -> ConfigResult<()> {
        let dir = tempdir()?;
        let config_path = dir.path().join(DEFAULT_CONFIG_FILE_NAME);
        fs::write(&config_path, "[meta]\nconfig_version = 2\n")?;
        fs::write(dir.path().join("noob_id"), "legacy-noob-id\n")?;
        fs::create_dir_all(dir.path().join("data"))?;
        let context = BootstrapChooserContext {
            default_config_path: config_path.clone(),
            reason: BootstrapChooserReason::DefaultConfigVersionMismatch {
                found_version: Some(2),
                expected_version: APP_CONFIG_VERSION,
            },
        };

        let launch = prepare_default_config_from_chooser(&context)?;
        let archive_root = dir.path().join("archive");
        let archive_entries = fs::read_dir(&archive_root)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, _>>()?;

        assert_eq!(archive_entries.len(), 1);
        let archive_dir = &archive_entries[0];
        assert!(archive_dir.join(DEFAULT_CONFIG_FILE_NAME).exists());
        assert!(archive_dir.join("noob_id").exists());
        assert!(archive_dir.join("data").exists());

        let config = AppConfig::load(&launch.config_path)?;
        assert_eq!(config.meta.config_version, APP_CONFIG_VERSION);
        assert!(launch.config_path.exists());
        Ok(())
    }
}
