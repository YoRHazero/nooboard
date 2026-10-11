use super::{publish, runtime::Progress};
use crate::{
    runtime,
    sftp::client::{Client, eof},
    *,
};
use russh_sftp::protocol::{FileAttributes, OpenFlags};
use std::path::Path;
use tokio::io::AsyncWriteExt;

pub(super) async fn run(
    client: &Client,
    remote: &RemotePath,
    directory: &Path,
    progress: &Progress,
) -> Result<()> {
    if !directory.is_absolute() {
        return Err(Error::InvalidPath);
    }
    let name = publish::filename(
        remote
            .as_str()
            .rsplit('/')
            .next()
            .ok_or(Error::InvalidPath)?,
    )?;
    let attrs = client.raw.lstat(remote.as_str()).await?.attrs;
    if attrs.permissions.map(|p| p & 0o170000) != Some(0o100000) {
        return Err(Error::NotFile);
    }
    let opened = client
        .raw
        .open(remote.as_str(), OpenFlags::READ, FileAttributes::default())
        .await;
    let handle = match opened {
        Ok(handle) => handle.handle,
        Err(error) => {
            if !matches!(error, russh_sftp::client::error::Error::Status(_)) {
                client.shutdown().await;
            }
            return Err(error.into());
        }
    };
    let temp = directory.join(format!(".nooboard-{}.part", runtime::id()));
    let mut created = false;
    let copied = async {
        let before = client.raw.fstat(handle.clone()).await?.attrs;
        if before.permissions.map(|p| p & 0o170000) != Some(0o100000) {
            return Err(Error::NotFile);
        }
        let mut options = tokio::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut target = options.open(&temp).await?;
        created = true;
        progress.temporary(temp.to_string_lossy().into_owned());
        if let Some(total) = before.size {
            progress.total(total);
        }
        let mut offset = 0;
        loop {
            progress.check_cancelled()?;
            match client.raw.read(handle.clone(), offset, 16 * 1024).await {
                Ok(data) => {
                    if data.data.is_empty() {
                        return Err(Error::Protocol("empty file data without EOF".into()));
                    }
                    target.write_all(&data.data).await?;
                    offset += data.data.len() as u64;
                    progress.bytes(offset);
                    if before.size.is_some_and(|n| offset > n) {
                        return Err(Error::SourceChanged);
                    }
                }
                Err(e) if eof(&e) => break,
                Err(e) => return Err(e.into()),
            }
        }
        let after = client.raw.fstat(handle.clone()).await?.attrs;
        if before.size != after.size
            || before.mtime != after.mtime
            || before.size.is_some_and(|n| n != offset)
        {
            return Err(Error::SourceChanged);
        }
        target.flush().await?;
        target.sync_all().await?;
        drop(target);
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
            publish::local(&temp, directory, name, progress)
                .await
                .map(|_| ())
        }
        Err(e) => Err(e),
    };
    // A successful publication is irrevocable even if cancellation arrives now.
    if created && tokio::fs::remove_file(&temp).await.is_ok() {
        progress.cleaned();
    }
    result
}
