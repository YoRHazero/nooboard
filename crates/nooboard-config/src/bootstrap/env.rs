use std::path::PathBuf;

pub const BOOTSTRAP_ENV_VAR: &str = "NOOBOARD_CONFIG";

pub fn config_override_path() -> Option<PathBuf> {
    std::env::var_os(BOOTSTRAP_ENV_VAR).map(PathBuf::from)
}
