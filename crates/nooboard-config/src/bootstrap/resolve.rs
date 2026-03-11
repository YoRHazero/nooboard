use std::path::Path;

use crate::{ConfigError, ConfigResult};

use super::env::config_override_path;
use super::paths::{default_config_path, repo_development_config_path};
use super::spec::{
    BootstrapChooserContext, BootstrapDecision, BootstrapLaunch, BootstrapMode, BootstrapRequest,
};

pub fn resolve_bootstrap(request: &BootstrapRequest) -> ConfigResult<BootstrapDecision> {
    resolve_bootstrap_with_override(request, config_override_path())
}

fn resolve_bootstrap_with_override(
    request: &BootstrapRequest,
    env_override: Option<std::path::PathBuf>,
) -> ConfigResult<BootstrapDecision> {
    if request.cli_config_path.is_some() && request.cli_use_repo_dev {
        return Err(ConfigError::InvalidBootstrap(
            "--config and --dev cannot be used together".to_string(),
        ));
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
        validate_existing_config_path(&config_path, "--dev")?;
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

    let default_config_path = default_config_path()?;
    if default_config_path.exists() {
        validate_existing_config_path(&default_config_path, "default config path")?;
        Ok(BootstrapDecision::Launch(BootstrapLaunch {
            mode: BootstrapMode::UserDefault,
            config_path: default_config_path,
        }))
    } else {
        Ok(BootstrapDecision::NeedsChooser(BootstrapChooserContext {
            default_config_path,
        }))
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
            cli_config_path: Some(PathBuf::from("/tmp/nooboard.toml")),
            cli_use_repo_dev: true,
        };

        let result = resolve_bootstrap(&request);
        assert!(matches!(result, Err(ConfigError::InvalidBootstrap(_))));
    }

    #[test]
    fn explicit_cli_config_must_exist() {
        let request = BootstrapRequest {
            cli_config_path: Some(PathBuf::from("/tmp/does-not-exist.toml")),
            cli_use_repo_dev: false,
        };

        let result = resolve_bootstrap_with_override(&request, None);
        assert!(matches!(result, Err(ConfigError::InvalidBootstrap(_))));
    }

    #[test]
    fn env_override_launches_existing_file() -> Result<(), ConfigError> {
        let temp = tempdir()?;
        let config_path = temp.path().join("nooboard.toml");
        fs::write(&config_path, "")?;

        let request = BootstrapRequest::default();
        let decision = resolve_bootstrap_with_override(&request, Some(config_path.clone()))?;

        assert_eq!(
            decision,
            BootstrapDecision::Launch(BootstrapLaunch {
                mode: BootstrapMode::ExplicitPath,
                config_path,
            })
        );
        Ok(())
    }
}
