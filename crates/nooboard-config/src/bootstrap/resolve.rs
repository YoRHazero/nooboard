use crate::defaults::APP_CONFIG_VERSION;
use std::path::Path;

use crate::{ConfigError, ConfigResult};

use super::env::config_override_path;
use super::paths::{default_config_path, repo_development_config_path};
use super::probe::{DefaultConfigState, inspect_default_config_state};
use super::spec::{
    BootstrapChooserContext, BootstrapChooserReason, BootstrapDecision, BootstrapLaunch,
    BootstrapMode, BootstrapRequest,
};

pub fn resolve_bootstrap(request: &BootstrapRequest) -> ConfigResult<BootstrapDecision> {
    resolve_bootstrap_with_paths(request, config_override_path(), None)
}

fn resolve_bootstrap_with_paths(
    request: &BootstrapRequest,
    env_override: Option<std::path::PathBuf>,
    default_config_override: Option<std::path::PathBuf>,
) -> ConfigResult<BootstrapDecision> {
    if request.cli_choose_config && (request.cli_config_path.is_some() || request.cli_use_repo_dev)
    {
        return Err(ConfigError::InvalidBootstrap(
            "--choose-config cannot be used together with --config or --dev".to_string(),
        ));
    }

    if request.cli_config_path.is_some() && request.cli_use_repo_dev {
        return Err(ConfigError::InvalidBootstrap(
            "--config and --dev cannot be used together".to_string(),
        ));
    }

    if request.cli_choose_config {
        return Ok(BootstrapDecision::NeedsChooser(BootstrapChooserContext {
            default_config_path: default_config_path()?,
            reason: BootstrapChooserReason::ExplicitChooserRequest,
        }));
    }

    if let Some(config_path) = request.cli_config_path.clone() {
        validate_existing_config_path(&config_path, "--config")?;
        return Ok(BootstrapDecision::Launch(BootstrapLaunch {
            mode: BootstrapMode::ExplicitPath,
            config_path,
        }));
    }

    if request.cli_use_repo_dev {
        let config_path = repo_development_config_path()?;
        return Ok(BootstrapDecision::Launch(BootstrapLaunch {
            mode: BootstrapMode::RepoDevelopment,
            config_path,
        }));
    }

    if let Some(config_path) = env_override {
        validate_existing_config_path(&config_path, super::env::BOOTSTRAP_ENV_VAR)?;
        return Ok(BootstrapDecision::Launch(BootstrapLaunch {
            mode: BootstrapMode::ExplicitPath,
            config_path,
        }));
    }

    let default_config_path = match default_config_override {
        Some(path) => path,
        None => default_config_path()?,
    };
    match inspect_default_config_state(&default_config_path, APP_CONFIG_VERSION)? {
        DefaultConfigState::Missing => {
            Ok(BootstrapDecision::NeedsChooser(BootstrapChooserContext {
                default_config_path,
                reason: BootstrapChooserReason::MissingDefaultConfig,
            }))
        }
        DefaultConfigState::Compatible => Ok(BootstrapDecision::Launch(BootstrapLaunch {
            mode: BootstrapMode::UserDefault,
            config_path: default_config_path,
        })),
        DefaultConfigState::Incompatible { found_version } => {
            Ok(BootstrapDecision::NeedsChooser(BootstrapChooserContext {
                default_config_path,
                reason: BootstrapChooserReason::DefaultConfigVersionMismatch {
                    found_version,
                    expected_version: APP_CONFIG_VERSION,
                },
            }))
        }
    }
}

fn validate_existing_config_path(path: &Path, source: &str) -> ConfigResult<()> {
    if !path.exists() {
        return Err(ConfigError::InvalidBootstrap(format!(
            "{source} config path does not exist: {}",
            path.display()
        )));
    }

    if !path.is_file() {
        return Err(ConfigError::InvalidBootstrap(format!(
            "{source} config path is not a file: {}",
            path.display()
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn cli_config_and_dev_conflict() {
        let request = BootstrapRequest {
            cli_choose_config: false,
            cli_config_path: Some(PathBuf::from("/tmp/nooboard.toml")),
            cli_use_repo_dev: true,
        };

        let result = resolve_bootstrap(&request);
        assert!(matches!(result, Err(ConfigError::InvalidBootstrap(_))));
    }

    #[test]
    fn explicit_cli_config_must_exist() {
        let request = BootstrapRequest {
            cli_choose_config: false,
            cli_config_path: Some(PathBuf::from("/tmp/does-not-exist.toml")),
            cli_use_repo_dev: false,
        };

        let result = resolve_bootstrap_with_paths(&request, None, None);
        assert!(matches!(result, Err(ConfigError::InvalidBootstrap(_))));
    }

    #[test]
    fn env_override_launches_existing_file() -> Result<(), ConfigError> {
        let temp = tempdir()?;
        let config_path = temp.path().join("nooboard.toml");
        fs::write(&config_path, "")?;

        let request = BootstrapRequest::default();
        let decision = resolve_bootstrap_with_paths(&request, Some(config_path.clone()), None)?;

        assert_eq!(
            decision,
            BootstrapDecision::Launch(BootstrapLaunch {
                mode: BootstrapMode::ExplicitPath,
                config_path,
            })
        );
        Ok(())
    }

    #[test]
    fn choose_config_conflicts_with_other_cli_bootstrap_flags() {
        let request = BootstrapRequest {
            cli_choose_config: true,
            cli_config_path: Some(PathBuf::from("/tmp/nooboard.toml")),
            cli_use_repo_dev: false,
        };

        let result = resolve_bootstrap_with_paths(&request, None, None);
        assert!(matches!(result, Err(ConfigError::InvalidBootstrap(_))));
    }

    #[test]
    fn choose_config_forces_chooser_even_when_env_override_exists() -> Result<(), ConfigError> {
        let temp = tempdir()?;
        let config_path = temp.path().join("nooboard.toml");
        fs::write(&config_path, "")?;

        let request = BootstrapRequest {
            cli_choose_config: true,
            ..BootstrapRequest::default()
        };
        let decision = resolve_bootstrap_with_paths(&request, Some(config_path), None)?;

        assert_eq!(
            decision,
            BootstrapDecision::NeedsChooser(BootstrapChooserContext {
                default_config_path: default_config_path()?,
                reason: BootstrapChooserReason::ExplicitChooserRequest,
            })
        );
        Ok(())
    }

    #[test]
    fn dev_mode_returns_repo_development_launch() -> Result<(), ConfigError> {
        let decision = resolve_bootstrap_with_paths(
            &BootstrapRequest {
                cli_use_repo_dev: true,
                ..BootstrapRequest::default()
            },
            None,
            None,
        )?;

        assert!(matches!(
            decision,
            BootstrapDecision::Launch(BootstrapLaunch {
                mode: BootstrapMode::RepoDevelopment,
                ..
            })
        ));
        Ok(())
    }

    #[test]
    fn missing_default_path_routes_to_chooser_with_missing_reason() -> Result<(), ConfigError> {
        let temp = tempdir()?;
        let config_path = temp.path().join("nooboard.toml");

        let decision = resolve_bootstrap_with_paths(
            &BootstrapRequest::default(),
            None,
            Some(config_path.clone()),
        )?;

        assert_eq!(
            decision,
            BootstrapDecision::NeedsChooser(BootstrapChooserContext {
                default_config_path: config_path,
                reason: BootstrapChooserReason::MissingDefaultConfig,
            })
        );
        Ok(())
    }

    #[test]
    fn incompatible_default_version_routes_to_chooser_with_version_reason()
    -> Result<(), ConfigError> {
        let temp = tempdir()?;
        let config_path = temp.path().join("nooboard.toml");
        fs::write(&config_path, "[meta]\nconfig_version = 2\n")?;

        let decision = resolve_bootstrap_with_paths(
            &BootstrapRequest::default(),
            None,
            Some(config_path.clone()),
        )?;

        assert_eq!(
            decision,
            BootstrapDecision::NeedsChooser(BootstrapChooserContext {
                default_config_path: config_path,
                reason: BootstrapChooserReason::DefaultConfigVersionMismatch {
                    found_version: Some(2),
                    expected_version: APP_CONFIG_VERSION,
                },
            })
        );
        Ok(())
    }
}
