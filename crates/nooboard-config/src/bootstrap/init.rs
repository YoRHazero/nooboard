use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use super::paths::DEFAULT_CONFIG_FILE_NAME;

pub fn resolve_init_output_path(output: Option<&Path>, cwd: &Path) -> PathBuf {
    match output {
        Some(path) => normalize_requested_path(path, cwd),
        None => cwd.join(DEFAULT_CONFIG_FILE_NAME),
    }
}

fn normalize_requested_path(path: &Path, cwd: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };

    if absolute.is_dir() {
        return absolute.join(DEFAULT_CONFIG_FILE_NAME);
    }

    if absolute.exists() {
        return absolute;
    }

    if looks_like_file_path(&absolute) {
        absolute
    } else {
        absolute.join(DEFAULT_CONFIG_FILE_NAME)
    }
}

fn looks_like_file_path(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| name == OsStr::new(DEFAULT_CONFIG_FILE_NAME))
        || path
            .extension()
            .is_some_and(|extension| extension == OsStr::new("toml"))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn missing_output_defaults_to_cwd_config_file() {
        let cwd = Path::new("/tmp/nooboard-init-test");
        assert_eq!(
            resolve_init_output_path(None, cwd),
            cwd.join(DEFAULT_CONFIG_FILE_NAME)
        );
    }

    #[test]
    fn existing_directory_resolves_to_default_config_file_name() {
        let dir = tempdir().expect("tempdir");
        assert_eq!(
            resolve_init_output_path(Some(dir.path()), Path::new("/unused")),
            dir.path().join(DEFAULT_CONFIG_FILE_NAME)
        );
    }

    #[test]
    fn explicit_toml_file_path_is_preserved() {
        let cwd = Path::new("/tmp/nooboard-init-test");
        let output = Path::new("custom/dev.toml");
        assert_eq!(
            resolve_init_output_path(Some(output), cwd),
            cwd.join(output)
        );
    }

    #[test]
    fn missing_path_without_toml_extension_is_treated_as_directory() {
        let cwd = Path::new("/tmp/nooboard-init-test");
        let output = Path::new("custom/location");
        assert_eq!(
            resolve_init_output_path(Some(output), cwd),
            cwd.join(output).join(DEFAULT_CONFIG_FILE_NAME)
        );
    }
}
