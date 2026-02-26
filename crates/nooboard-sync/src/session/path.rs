use std::path::{Path, PathBuf};

use tokio::fs;

use crate::error::FileReceiveError;

pub(crate) fn sanitize_file_name(raw: &str) -> Result<String, FileReceiveError> {
    if raw.trim().is_empty() || raw.contains('/') || raw.contains('\\') || raw.contains("..") {
        return Err(FileReceiveError::InvalidFileName(raw.to_string()));
    }

    let parsed = Path::new(raw)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| FileReceiveError::InvalidFileName(raw.to_string()))?;

    if parsed != raw {
        return Err(FileReceiveError::InvalidFileName(raw.to_string()));
    }

    Ok(parsed.to_string())
}

pub(crate) async fn resolve_final_path(
    download_dir: &Path,
    file_name: &str,
) -> Result<PathBuf, FileReceiveError> {
    let candidate = download_dir.join(file_name);
    if !fs::try_exists(&candidate).await? {
        return Ok(candidate);
    }

    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(file_name);
    let extension = Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| format!(".{value}"))
        .unwrap_or_default();

    for index in 1..=10_000 {
        let candidate = download_dir.join(format!("{stem} ({index}){extension}"));
        if !fs::try_exists(&candidate).await? {
            return Ok(candidate);
        }
    }

    Err(FileReceiveError::InvalidFileName(file_name.to_string()))
}

pub(crate) fn ensure_inside_download_dir(
    download_dir: &Path,
    path: &Path,
) -> Result<(), FileReceiveError> {
    if path.starts_with(download_dir) {
        Ok(())
    } else {
        Err(FileReceiveError::UnsafePath)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_rejects_traversal_name() {
        let error =
            sanitize_file_name("../../evil.txt").expect_err("traversal name must be rejected");
        assert!(matches!(error, FileReceiveError::InvalidFileName(_)));
    }

    #[test]
    fn sanitize_accepts_normal_name() {
        let sanitized =
            sanitize_file_name("hello.txt").expect("normal file name should be accepted");
        assert_eq!(sanitized, "hello.txt");
    }

    #[tokio::test]
    async fn resolve_final_path_adds_suffix_when_file_exists() {
        let dir = tempfile::tempdir().expect("tempdir must be created");
        let first = dir.path().join("a.txt");
        tokio::fs::write(&first, b"1")
            .await
            .expect("seed file should be created");

        let next = resolve_final_path(dir.path(), "a.txt")
            .await
            .expect("next candidate should be resolved");
        assert_eq!(next, dir.path().join("a (1).txt"));
    }

    #[test]
    fn ensure_inside_download_dir_rejects_outside_path() {
        let dir = tempfile::tempdir().expect("tempdir must be created");
        let outside = dir
            .path()
            .parent()
            .expect("temp dir must have parent")
            .join("outside.txt");

        let error = ensure_inside_download_dir(dir.path(), &outside)
            .expect_err("outside path should be rejected");
        assert!(matches!(error, FileReceiveError::UnsafePath));
    }
}
