use super::{publish, runtime::Progress};
use crate::{
    runtime,
    sftp::{client::Client, path},
    *,
};
use russh_sftp::protocol::{FileAttributes, OpenFlags};
use std::path::Path;
use tokio::io::AsyncReadExt;

pub(super) async fn run(
    client: &Client,
    local: &Path,
    directory: &RemotePath,
    progress: &Progress,
) -> Result<()> {
    if !local.is_absolute() {
        return Err(Error::InvalidPath);
    }
    let name = publish::filename(
        local
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or(Error::InvalidPath)?,
    )?;
    if !tokio::fs::symlink_metadata(local).await?.is_file() {
        return Err(Error::NotFile);
    }
    let mut source = tokio::fs::File::open(local).await?;
    let before = source.metadata().await?;
    if !before.is_file() {
        return Err(Error::NotFile);
    }
    let directory = path::validate(client, directory, ".").await?;
    let temp = directory.child(&format!(".nooboard-{}.part", runtime::id()))?;
    progress.check_cancelled()?;
    let opened = client
        .raw
        .open(
            temp.as_str(),
            OpenFlags::CREATE | OpenFlags::EXCLUDE | OpenFlags::WRITE,
            FileAttributes {
                permissions: Some(0o600),
                ..Default::default()
            },
        )
        .await;
    let handle = match opened {
        Ok(handle) => handle.handle,
        Err(error) => {
            if !matches!(error, russh_sftp::client::error::Error::Status(_)) {
                // OPEN may have created the file even when its handle reply was
                // lost. Record the pathname and close the transport to release
                // any unknown remote handle; do not guess ownership and unlink.
                progress.temporary(temp.as_str().into());
                client.shutdown().await;
            }
            return Err(error.into());
        }
    };
    progress.temporary(temp.as_str().into());
    progress.total(before.len());
    let copied = async {
        let mut offset = 0;
        let mut buffer = vec![0; 16 * 1024];
        loop {
            progress.check_cancelled()?;
            let len = source.read(&mut buffer).await?;
            if len == 0 {
                break;
            }
            client
                .raw
                .write(handle.clone(), offset, buffer[..len].to_vec())
                .await?;
            offset += len as u64;
            progress.bytes(offset);
            if offset > before.len() {
                return Err(Error::SourceChanged);
            }
        }
        let after = source.metadata().await?;
        if offset != before.len()
            || before.len() != after.len()
            || before.modified().ok() != after.modified().ok()
        {
            return Err(Error::SourceChanged);
        }
        Ok(())
    }
    .await;
    let closed = client
        .raw
        .close(handle)
        .await
        .map(|_| ())
        .map_err(Error::from);
    let result = match copied.and(closed) {
        Ok(()) => {
            progress.finalizing();
            publish::remote(client, &temp, &directory, name, progress)
                .await
                .map(|_| ())
        }
        Err(e) => Err(e),
    };
    if matches!(result, Err(Error::Unconfirmed)) {
        return result;
    }
    if result.is_ok() || client.raw.remove(temp.as_str()).await.is_ok() {
        progress.cleaned();
    }
    result
}
