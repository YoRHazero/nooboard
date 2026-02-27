use std::fs;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::{AppError, AppResult};

pub(super) fn resolve_or_init_node_id(path: &Path) -> AppResult<String> {
    match fs::read_to_string(path) {
        Ok(raw) => {
            let node_id = raw.trim();
            if !node_id.is_empty() {
                return Ok(node_id.to_string());
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    regenerate_node_id(path)
}

pub(super) fn regenerate_node_id(path: &Path) -> AppResult<String> {
    let parent = path.parent().ok_or_else(|| {
        AppError::InvalidConfig(format!(
            "identity.noob_id_file `{}` has no parent",
            path.display()
        ))
    })?;
    fs::create_dir_all(parent)?;

    let generated = Uuid::now_v7().to_string();
    fs::write(path, format!("{generated}\n"))?;
    Ok(generated)
}

pub(super) fn absolutize_if_relative(path: &mut PathBuf, base_dir: &Path) {
    if path.is_relative() {
        *path = base_dir.join(path.clone());
    }
}
