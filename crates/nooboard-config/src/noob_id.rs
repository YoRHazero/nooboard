use std::fs;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::{ConfigError, ConfigResult};

pub(crate) fn resolve_or_init_noob_id(path: &Path) -> ConfigResult<String> {
    match fs::read_to_string(path) {
        Ok(raw) => {
            let noob_id = raw.trim();
            if !noob_id.is_empty() {
                return Ok(noob_id.to_string());
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    regenerate_noob_id(path)
}

pub(crate) fn regenerate_noob_id(path: &Path) -> ConfigResult<String> {
    let parent = path.parent().ok_or_else(|| {
        ConfigError::InvalidConfig(format!(
            "identity.noob_id_file `{}` has no parent",
            path.display()
        ))
    })?;
    fs::create_dir_all(parent)?;

    let generated = Uuid::now_v7().to_string();
    fs::write(path, format!("{generated}\n"))?;
    Ok(generated)
}

pub(crate) fn absolutize_if_relative(path: &mut PathBuf, base_dir: &Path) {
    if path.is_relative() {
        *path = base_dir.join(path.clone());
    }
}
