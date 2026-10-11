use super::runtime::Progress;
use crate::{sftp::client::Client, *};
use std::path::{Path, PathBuf};

pub(super) fn filename(value: &str) -> Result<&str> {
    if value.is_empty()
        || value == "."
        || value == ".."
        || value.contains(['/', '\\', '\0', ':'])
        || value.chars().any(char::is_control)
    {
        Err(Error::InvalidPath)
    } else {
        Ok(value)
    }
}
pub(super) fn numbered(name: &str, number: usize) -> String {
    if number == 0 {
        return name.into();
    }
    match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => format!("{stem} ({number}).{extension}"),
        _ => format!("{name} ({number})"),
    }
}
pub(super) async fn local(
    temp: &Path,
    directory: &Path,
    name: &str,
    progress: &Progress,
) -> Result<PathBuf> {
    for number in 0..10000 {
        progress.check_cancelled()?;
        let target = directory.join(numbered(name, number));
        progress.target(target.to_string_lossy().into_owned());
        // Unlike rename(), hard_link cannot replace an existing destination. Both
        // paths are on the same filesystem; unsupported filesystems fail explicitly.
        match tokio::fs::hard_link(temp, &target).await {
            Ok(()) => return Ok(target),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err(Error::Limit)
}
pub(super) async fn remote(
    client: &Client,
    temp: &RemotePath,
    directory: &RemotePath,
    name: &str,
    progress: &Progress,
) -> Result<RemotePath> {
    for number in 0..10000 {
        progress.check_cancelled()?;
        let target = directory.child(&numbered(name, number))?;
        progress.target(target.as_str().into());
        // SSH_FXP_RENAME v3 explicitly forbids replacing an existing destination.
        // Do not use posix-rename@openssh.com, which has overwrite semantics.
        match client.raw.rename(temp.as_str(), target.as_str()).await {
            Ok(_) => return Ok(target),
            Err(russh_sftp::client::error::Error::Status(status)) => {
                match client.raw.lstat(target.as_str()).await {
                    Ok(_) => continue,
                    Err(e) if matches!(Error::from(e.clone()), Error::NotFound) => {
                        return Err(russh_sftp::client::error::Error::Status(status).into());
                    }
                    Err(e) => return Err(e.into()),
                }
            }
            // No response cannot tell us whether the rename took effect. Never
            // replay it or remove either pathname based on a guess.
            Err(_) => return Err(Error::Unconfirmed),
        }
    }
    Err(Error::Limit)
}
