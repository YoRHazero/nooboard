use super::{DriverEvent, ssh};
use crate::*;
use russh_sftp::{
    client::{error::Error as SftpError, rawsession::RawSftpSession},
    protocol::{FileAttributes, OpenFlags, Packet, StatusCode},
};
use std::{
    future::Future,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::mpsc,
    time::timeout,
};

pub(crate) struct Sftp {
    raw: RawSftpSession,
    atomic_replace: bool,
    timeout: Duration,
}
fn map_error(error: SftpError) -> TerminalError {
    let kind = match &error {
        SftpError::Status(status) => match status.status_code {
            StatusCode::NoSuchFile => TerminalErrorKind::PathNotFound,
            StatusCode::PermissionDenied => TerminalErrorKind::PermissionDenied,
            StatusCode::OpUnsupported => TerminalErrorKind::Unsupported,
            StatusCode::NoConnection | StatusCode::ConnectionLost => {
                TerminalErrorKind::ConnectionLost
            }
            _ => TerminalErrorKind::Other,
        },
        _ => TerminalErrorKind::Other,
    };
    TerminalError::new(kind, error.to_string())
}
fn is_eof(error: &SftpError) -> bool {
    matches!(error, SftpError::Status(s) if s.status_code == StatusCode::Eof)
}
fn uncertain(error: &SftpError) -> bool {
    !matches!(error, SftpError::Status(_))
}
fn remote_error(error: TerminalError, path: &RemotePath) -> TerminalError {
    error.at(FileOperationPath::Remote(path.clone()))
}
fn local_error(error: TerminalError, path: &Path) -> TerminalError {
    error.at(FileOperationPath::Local(path.to_owned()))
}
fn check(stop: &CancellationToken) -> Result<()> {
    if stop.is_cancelled() {
        Err(TerminalError::cancelled())
    } else {
        Ok(())
    }
}
fn kind(attrs: &FileAttributes) -> RemoteEntryKind {
    match attrs.permissions.map(|p| p & 0o170000) {
        None => RemoteEntryKind::Unknown,
        Some(0o100000) => RemoteEntryKind::File,
        Some(0o040000) => RemoteEntryKind::Directory,
        Some(0o120000) => RemoteEntryKind::SymbolicLink,
        _ => RemoteEntryKind::Other,
    }
}
impl Sftp {
    async fn raw_call<T>(
        &self,
        future: impl Future<Output = std::result::Result<T, SftpError>>,
    ) -> std::result::Result<T, SftpError> {
        timeout(self.timeout, future)
            .await
            .map_err(|_| SftpError::Timeout)?
    }
    async fn call<T>(
        &self,
        future: impl Future<Output = std::result::Result<T, SftpError>>,
    ) -> Result<T> {
        self.raw_call(future).await.map_err(map_error)
    }
    async fn metadata(&self, path: &RemotePath) -> Result<Option<FileAttributes>> {
        match self.raw_call(self.raw.lstat(path.as_str())).await {
            Ok(attrs) => Ok(Some(attrs.attrs)),
            Err(SftpError::Status(s)) if s.status_code == StatusCode::NoSuchFile => Ok(None),
            Err(error) => Err(remote_error(map_error(error), path)),
        }
    }
    async fn entries(
        &self,
        path: &RemotePath,
        stop: &CancellationToken,
    ) -> Result<Vec<russh_sftp::protocol::File>> {
        check(stop)?;
        let handle = self.call(self.raw.opendir(path.as_str())).await?.handle;
        let result = async {
            let mut entries = Vec::new();
            loop {
                check(stop)?;
                match self.raw_call(self.raw.readdir(&handle)).await {
                    Ok(batch) => {
                        for file in batch.files {
                            if file.filename != "." && file.filename != ".." {
                                path.join(&file.filename)?;
                                entries.push(file);
                            }
                        }
                    }
                    Err(error) if is_eof(&error) => return Ok(entries),
                    Err(error) => return Err(map_error(error)),
                }
            }
        }
        .await;
        let closed = self.call(self.raw.close(handle)).await;
        let entries = result.map_err(|e| remote_error(e, path))?;
        closed?;
        Ok(entries)
    }
    async fn commit(
        &self,
        temp: &RemotePath,
        target: &RemotePath,
        policy: FileConflictPolicy,
    ) -> Result<bool> {
        let existing = self.metadata(target).await?.map(|a| kind(&a));
        if skip_or_fail(existing, false, policy).map_err(|e| remote_error(e, target))? {
            return Ok(false);
        }
        let result = if policy == FileConflictPolicy::Overwrite && self.atomic_replace {
            let mut bytes = Vec::new();
            for path in [temp, target] {
                let name = path.as_str().as_bytes();
                let length = u32::try_from(name.len())
                    .map_err(|_| TerminalError::invalid("remote path exceeds SFTP length"))?;
                bytes.extend(length.to_be_bytes());
                bytes.extend(name);
            }
            match self
                .raw_call(self.raw.extended("posix-rename@openssh.com", bytes))
                .await
            {
                Ok(Packet::Status(status)) if status.status_code == StatusCode::Ok => Ok(()),
                Ok(Packet::Status(status)) => Err(SftpError::Status(status)),
                Ok(_) => Err(SftpError::UnexpectedPacket),
                Err(error) => Err(error),
            }
        } else {
            if existing.is_some() {
                return Err(remote_error(
                    TerminalError::new(
                        TerminalErrorKind::Unsupported,
                        "server cannot atomically replace an existing file",
                    ),
                    target,
                ));
            }
            self.raw_call(self.raw.rename(temp.as_str(), target.as_str()))
                .await
                .map(|_| ())
        };
        match result {
            Ok(()) => Ok(true),
            Err(error) if uncertain(&error) => Err(remote_error(
                TerminalError::new(
                    TerminalErrorKind::CommitUnconfirmed,
                    "server did not confirm the final file commit",
                ),
                target,
            )),
            Err(error) => {
                if self.metadata(target).await?.is_some() && policy == FileConflictPolicy::Skip {
                    return Ok(false);
                }
                Err(remote_error(map_error(error), target))
            }
        }
    }
}
pub(crate) async fn session(
    connection: &Arc<ssh::Connection>,
    options: &TerminalServiceOptions,
) -> Result<Arc<Sftp>> {
    connection
        .sftp
        .get_or_try_init(|| async {
            let duration = options.io.request_timeout;
            let mut channel =
                ssh::request(duration, connection.client.channel_open_session()).await?;
            ssh::request(duration, channel.request_subsystem(true, "sftp")).await?;
            ssh::confirmation(&mut channel, duration, None).await?;
            let raw = RawSftpSession::new_with_config(
                channel.into_stream(),
                russh_sftp::client::Config {
                    request_timeout_secs: duration
                        .as_secs()
                        .saturating_add(u64::from(duration.subsec_nanos() != 0)),
                    ..Default::default()
                },
            );
            let version = timeout(duration, raw.init())
                .await
                .map_err(|_| {
                    TerminalError::new(TerminalErrorKind::Other, "SFTP initialization timed out")
                })?
                .map_err(map_error)?;
            Ok(Arc::new(Sftp {
                raw,
                atomic_replace: version
                    .extensions
                    .get("posix-rename@openssh.com")
                    .is_some_and(|v| v == "1"),
                timeout: duration,
            }))
        })
        .await
        .cloned()
}
pub(crate) async fn list(
    connection: Arc<ssh::Connection>,
    options: TerminalServiceOptions,
    path: Option<RemotePath>,
    stop: CancellationToken,
) -> Result<RemoteDirectoryListing> {
    check(&stop)?;
    let sftp = session(&connection, &options).await?;
    check(&stop)?;
    let resolved = sftp
        .call(
            sftp.raw
                .realpath(path.as_ref().map(RemotePath::as_str).unwrap_or(".")),
        )
        .await?;
    let path = RemotePath::new(
        resolved
            .files
            .first()
            .ok_or_else(|| TerminalError::new(TerminalErrorKind::Other, "empty SFTP path reply"))?
            .filename
            .clone(),
    )?;
    let entries = sftp
        .entries(&path, &stop)
        .await?
        .into_iter()
        .map(|entry| RemoteDirectoryEntry {
            name: entry.filename,
            kind: kind(&entry.attrs),
            size: entry.attrs.size,
            modified_at: entry.attrs.mtime.map(i64::from),
        })
        .collect();
    Ok(RemoteDirectoryListing { path, entries })
}
pub(crate) async fn mkdir(
    connection: Arc<ssh::Connection>,
    options: TerminalServiceOptions,
    path: RemotePath,
    stop: CancellationToken,
) -> Result<()> {
    let sftp = session(&connection, &options).await?;
    check(&stop)?;
    if sftp.metadata(&path).await?.is_some() {
        return Err(remote_error(
            TerminalError::new(TerminalErrorKind::Conflict, "path already exists"),
            &path,
        ));
    }
    sftp.call(sftp.raw.mkdir(path.as_str(), FileAttributes::default()))
        .await
        .map(|_| ())
        .map_err(|e| remote_error(e, &path))
}
pub(crate) async fn rename(
    connection: Arc<ssh::Connection>,
    options: TerminalServiceOptions,
    source: RemotePath,
    destination: RemotePath,
    stop: CancellationToken,
) -> Result<()> {
    let sftp = session(&connection, &options).await?;
    check(&stop)?;
    if sftp.metadata(&destination).await?.is_some() {
        return Err(remote_error(
            TerminalError::new(TerminalErrorKind::Conflict, "destination already exists"),
            &destination,
        ));
    }
    match sftp
        .raw_call(sftp.raw.rename(source.as_str(), destination.as_str()))
        .await
    {
        Ok(_) => Ok(()),
        Err(error) if uncertain(&error) => Err(remote_error(
            TerminalError::new(
                TerminalErrorKind::CommitUnconfirmed,
                "rename result was not confirmed",
            ),
            &destination,
        )),
        Err(error) => Err(remote_error(map_error(error), &source)),
    }
}
fn skip_or_fail(
    existing: Option<RemoteEntryKind>,
    directory: bool,
    policy: FileConflictPolicy,
) -> Result<bool> {
    let Some(existing) = existing else {
        return Ok(false);
    };
    if existing == RemoteEntryKind::SymbolicLink {
        return if policy == FileConflictPolicy::Skip {
            Ok(true)
        } else {
            Err(TerminalError::new(
                TerminalErrorKind::Conflict,
                "destination is a symbolic link",
            ))
        };
    }
    if directory && existing == RemoteEntryKind::Directory {
        return Ok(false);
    }
    if directory || existing != RemoteEntryKind::File {
        return Err(TerminalError::new(
            TerminalErrorKind::Conflict,
            "source and destination types differ",
        ));
    }
    match policy {
        FileConflictPolicy::Fail => Err(TerminalError::new(
            TerminalErrorKind::Conflict,
            "destination exists",
        )),
        FileConflictPolicy::Skip => Ok(true),
        FileConflictPolicy::Overwrite => Ok(false),
    }
}
async fn local_kind(path: &Path) -> Result<Option<RemoteEntryKind>> {
    match tokio::fs::symlink_metadata(path).await {
        Ok(m) => Ok(Some(if m.is_symlink() {
            RemoteEntryKind::SymbolicLink
        } else if m.is_dir() {
            RemoteEntryKind::Directory
        } else if m.is_file() {
            RemoteEntryKind::File
        } else {
            RemoteEntryKind::Other
        })),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(local_error(e.into(), path)),
    }
}
fn local_child(parent: &Path, name: &str) -> Result<PathBuf> {
    if name.contains(['/', '\\', '\0'])
        || name == "."
        || name == ".."
        || name.is_empty()
        || Path::new(name).components().count() != 1
        || !matches!(
            Path::new(name).components().next(),
            Some(std::path::Component::Normal(_))
        )
    {
        return Err(TerminalError::invalid(
            "server returned an unsafe local filename",
        ));
    }
    Ok(parent.join(name))
}
pub(crate) struct FileCompletion {
    pub result: Result<()>,
    pub progress: FileOperationProgress,
    pub cleanup_issues: Vec<FileCleanupIssue>,
}
struct Runner {
    sftp: Arc<Sftp>,
    id: FileOperationRunId,
    stop: CancellationToken,
    options: TerminalServiceOptions,
    events: mpsc::UnboundedSender<DriverEvent>,
    progress: FileOperationProgress,
    temporary: Vec<FileOperationPath>,
    uncertain_temps: Vec<FileCleanupIssue>,
    last_progress: Option<tokio::time::Instant>,
}
impl Runner {
    fn progress(&mut self) {
        if self
            .last_progress
            .is_some_and(|last| last.elapsed() < self.options.progress_interval)
        {
            return;
        }
        self.last_progress = Some(tokio::time::Instant::now());
        let _ = self.events.send(DriverEvent::FileProgress {
            id: self.id,
            progress: self.progress.clone(),
        });
    }
    async fn upload_file(
        &mut self,
        item: &FileTransferItem,
        policy: FileConflictPolicy,
    ) -> Result<()> {
        check(&self.stop)?;
        if skip_or_fail(
            self.sftp
                .metadata(&item.remote_path)
                .await?
                .map(|a| kind(&a)),
            false,
            policy,
        )
        .map_err(|error| remote_error(error, &item.remote_path))?
        {
            self.progress.skipped_entries += 1;
            return Ok(());
        }
        let temp = item.remote_path.temporary_sibling()?;
        let mut open = tokio::fs::OpenOptions::new();
        open.read(true);
        #[cfg(unix)]
        open.custom_flags(libc::O_NOFOLLOW);
        let mut source = open
            .open(&item.local_path)
            .await
            .map_err(|e| local_error(e.into(), &item.local_path))?;
        if !source.metadata().await?.is_file() {
            return Err(local_error(
                TerminalError::new(
                    TerminalErrorKind::Unsupported,
                    "source is not a regular file",
                ),
                &item.local_path,
            ));
        }
        let handle = match self
            .sftp
            .raw_call(self.sftp.raw.open(
                temp.as_str(),
                OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::EXCLUDE,
                FileAttributes::default(),
            ))
            .await
        {
            Ok(opened) => opened.handle,
            Err(error) => {
                if uncertain(&error) {
                    self.uncertain_temps.push(FileCleanupIssue {
                        path: FileOperationPath::Remote(temp.clone()),
                        error: TerminalError::new(
                            TerminalErrorKind::Other,
                            "temporary file creation was not confirmed; it may remain on the server",
                        ),
                    });
                }
                return Err(remote_error(map_error(error), &temp));
            }
        };
        self.temporary.push(FileOperationPath::Remote(temp.clone()));
        let result = async {
            let mut offset = 0;
            let mut buffer = vec![0; self.options.transfer_buffer_bytes];
            loop {
                check(&self.stop)?;
                let count = source.read(&mut buffer).await?;
                if count == 0 {
                    break;
                }
                self.sftp
                    .call(
                        self.sftp
                            .raw
                            .write(&handle, offset, buffer[..count].to_vec()),
                    )
                    .await?;
                offset += count as u64;
                self.progress.transferred_bytes += count as u64;
                self.progress();
            }
            Ok(())
        }
        .await;
        let close = self.sftp.call(self.sftp.raw.close(handle)).await;
        result.map_err(|e| remote_error(e, &item.remote_path))?;
        close?;
        check(&self.stop)?;
        if self.sftp.commit(&temp, &item.remote_path, policy).await? {
            self.temporary
                .retain(|p| !matches!(p, FileOperationPath::Remote(path) if path == &temp));
            self.progress.completed_entries += 1;
        } else {
            self.progress.skipped_entries += 1;
        }
        Ok(())
    }
    async fn download_file(
        &mut self,
        item: &FileTransferItem,
        policy: FileConflictPolicy,
    ) -> Result<()> {
        check(&self.stop)?;
        if skip_or_fail(local_kind(&item.local_path).await?, false, policy)
            .map_err(|error| local_error(error, &item.local_path))?
        {
            self.progress.skipped_entries += 1;
            return Ok(());
        }
        let parent = item
            .local_path
            .parent()
            .ok_or_else(|| TerminalError::invalid("destination has no parent"))?;
        let temp = parent.join(format!(".nooboard-{}.tmp", uuid::Uuid::new_v4()));
        let mut output = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .await
            .map_err(|e| local_error(e.into(), &temp))?;
        self.temporary.push(FileOperationPath::Local(temp.clone()));
        let handle = self
            .sftp
            .call(self.sftp.raw.open(
                item.remote_path.as_str(),
                OpenFlags::READ,
                FileAttributes::default(),
            ))
            .await?
            .handle;
        let result = async {
            let mut offset = 0;
            let length = u32::try_from(self.options.transfer_buffer_bytes)
                .map_err(|_| TerminalError::invalid("transfer buffer exceeds SFTP request size"))?;
            loop {
                check(&self.stop)?;
                match self
                    .sftp
                    .raw_call(self.sftp.raw.read(&handle, offset, length))
                    .await
                {
                    Ok(data) => {
                        if data.data.is_empty() {
                            return Err(TerminalError::new(
                                TerminalErrorKind::Other,
                                "empty SFTP read without EOF",
                            ));
                        }
                        output.write_all(&data.data).await?;
                        offset += data.data.len() as u64;
                        self.progress.transferred_bytes += data.data.len() as u64;
                        self.progress();
                    }
                    Err(error) if is_eof(&error) => break,
                    Err(error) => return Err(map_error(error)),
                }
            }
            output.flush().await?;
            Ok(())
        }
        .await;
        let close = self.sftp.call(self.sftp.raw.close(handle)).await;
        drop(output);
        result.map_err(|e| remote_error(e, &item.remote_path))?;
        close?;
        check(&self.stop)?;
        if skip_or_fail(local_kind(&item.local_path).await?, false, policy)
            .map_err(|error| local_error(error, &item.local_path))?
        {
            self.progress.skipped_entries += 1;
            return Ok(());
        }
        if policy == FileConflictPolicy::Overwrite {
            tokio::fs::rename(&temp, &item.local_path)
                .await
                .map_err(|e| local_error(e.into(), &item.local_path))?;
            self.temporary
                .retain(|p| !matches!(p, FileOperationPath::Local(path) if path == &temp));
        } else {
            match tokio::fs::hard_link(&temp, &item.local_path).await {
                Ok(()) => {}
                Err(error)
                    if error.kind() == std::io::ErrorKind::AlreadyExists
                        && policy == FileConflictPolicy::Skip =>
                {
                    self.progress.skipped_entries += 1;
                    return Ok(());
                }
                Err(error) => return Err(local_error(error.into(), &item.local_path)),
            }
        }
        self.progress.completed_entries += 1;
        Ok(())
    }
    async fn transfer(
        &mut self,
        items: Vec<FileTransferItem>,
        policy: FileConflictPolicy,
        upload: bool,
    ) -> Result<()> {
        enum Work {
            Enter(FileTransferItem),
            FinishDirectory,
        }
        let mut stack: Vec<_> = items.into_iter().rev().map(Work::Enter).collect();
        while let Some(work) = stack.pop() {
            check(&self.stop)?;
            let Work::Enter(item) = work else {
                self.progress.completed_entries += 1;
                self.progress();
                continue;
            };
            let source_kind = if upload {
                local_kind(&item.local_path).await?
            } else {
                self.sftp
                    .metadata(&item.remote_path)
                    .await?
                    .map(|a| kind(&a))
            };
            match source_kind {
                Some(RemoteEntryKind::File) => {
                    if upload {
                        self.upload_file(&item, policy).await?;
                    } else {
                        self.download_file(&item, policy).await?;
                    }
                }
                Some(RemoteEntryKind::Directory) => {
                    let dest = if upload {
                        self.sftp
                            .metadata(&item.remote_path)
                            .await?
                            .map(|a| kind(&a))
                    } else {
                        local_kind(&item.local_path).await?
                    };
                    if skip_or_fail(dest, true, policy).map_err(|error| {
                        if upload {
                            remote_error(error, &item.remote_path)
                        } else {
                            local_error(error, &item.local_path)
                        }
                    })? {
                        self.progress.skipped_entries += 1;
                        self.progress();
                        continue;
                    }
                    if dest.is_none() {
                        if upload {
                            self.sftp
                                .call(
                                    self.sftp.raw.mkdir(
                                        item.remote_path.as_str(),
                                        FileAttributes::default(),
                                    ),
                                )
                                .await?;
                        } else {
                            tokio::fs::create_dir(&item.local_path)
                                .await
                                .map_err(|e| local_error(e.into(), &item.local_path))?;
                        }
                    }
                    stack.push(Work::FinishDirectory);
                    if upload {
                        let mut entries = tokio::fs::read_dir(&item.local_path).await?;
                        while let Some(entry) = entries.next_entry().await? {
                            check(&self.stop)?;
                            let name = entry.file_name().into_string().map_err(|_| {
                                TerminalError::new(
                                    TerminalErrorKind::Unsupported,
                                    "non-UTF-8 filename cannot be represented by RemotePath",
                                )
                            })?;
                            stack.push(Work::Enter(FileTransferItem {
                                local_path: entry.path(),
                                remote_path: item.remote_path.join(&name)?,
                            }));
                        }
                    } else {
                        for entry in self.sftp.entries(&item.remote_path, &self.stop).await? {
                            stack.push(Work::Enter(FileTransferItem {
                                local_path: local_child(&item.local_path, &entry.filename)?,
                                remote_path: item.remote_path.join(&entry.filename)?,
                            }));
                        }
                    }
                }
                None => {
                    let e = TerminalError::new(
                        TerminalErrorKind::PathNotFound,
                        "source does not exist",
                    );
                    return Err(if upload {
                        local_error(e, &item.local_path)
                    } else {
                        remote_error(e, &item.remote_path)
                    });
                }
                _ => {
                    let e = TerminalError::new(
                        TerminalErrorKind::Unsupported,
                        "symbolic links and special files cannot be transferred",
                    );
                    return Err(if upload {
                        local_error(e, &item.local_path)
                    } else {
                        remote_error(e, &item.remote_path)
                    });
                }
            }
            self.progress();
        }
        Ok(())
    }
    async fn delete(&mut self, paths: Vec<RemotePath>) -> Result<()> {
        let mut stack: Vec<_> = paths.into_iter().rev().map(|p| (p, false)).collect();
        while let Some((path, after_children)) = stack.pop() {
            check(&self.stop)?;
            if after_children {
                self.sftp
                    .call(self.sftp.raw.rmdir(path.as_str()))
                    .await
                    .map_err(|e| remote_error(e, &path))?;
            } else {
                let attrs = self.sftp.metadata(&path).await?.ok_or_else(|| {
                    remote_error(
                        TerminalError::new(
                            TerminalErrorKind::PathNotFound,
                            "delete target does not exist",
                        ),
                        &path,
                    )
                })?;
                if kind(&attrs) == RemoteEntryKind::Directory {
                    let entries = self.sftp.entries(&path, &self.stop).await?;
                    stack.push((path.clone(), true));
                    for entry in entries {
                        stack.push((path.join(&entry.filename)?, false));
                    }
                    continue;
                }
                self.sftp
                    .call(self.sftp.raw.remove(path.as_str()))
                    .await
                    .map_err(|e| remote_error(e, &path))?;
            }
            self.progress.completed_entries += 1;
            self.progress();
        }
        Ok(())
    }
    async fn cleanup(&mut self) -> Vec<FileCleanupIssue> {
        let mut issues = std::mem::take(&mut self.uncertain_temps);
        for path in std::mem::take(&mut self.temporary) {
            let result = match &path {
                FileOperationPath::Local(p) => match tokio::fs::remove_file(p).await {
                    Ok(()) => Ok(()),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                    Err(e) => Err(e.into()),
                },
                FileOperationPath::Remote(p) => {
                    match self.sftp.raw_call(self.sftp.raw.remove(p.as_str())).await {
                        Ok(_) => Ok(()),
                        Err(SftpError::Status(s)) if s.status_code == StatusCode::NoSuchFile => {
                            Ok(())
                        }
                        Err(e) => Err(map_error(e)),
                    }
                }
            };
            if let Err(error) = result {
                issues.push(FileCleanupIssue { path, error });
            }
        }
        issues
    }
}
pub(crate) async fn run(
    id: FileOperationRunId,
    spec: FileOperationSpec,
    connection: Arc<ssh::Connection>,
    options: TerminalServiceOptions,
    stop: CancellationToken,
    events: mpsc::UnboundedSender<DriverEvent>,
) -> FileCompletion {
    let sftp = match session(&connection, &options).await {
        Ok(s) => s,
        Err(e) => {
            return FileCompletion {
                result: Err(e),
                progress: Default::default(),
                cleanup_issues: vec![],
            };
        }
    };
    let _ = events.send(DriverEvent::FileStarted(id));
    let mut runner = Runner {
        sftp,
        id,
        stop,
        options,
        events,
        progress: Default::default(),
        temporary: vec![],
        uncertain_temps: vec![],
        last_progress: None,
    };
    let result = match spec {
        FileOperationSpec::Upload {
            items,
            conflict_policy,
        } => runner.transfer(items, conflict_policy, true).await,
        FileOperationSpec::Download {
            items,
            conflict_policy,
        } => runner.transfer(items, conflict_policy, false).await,
        FileOperationSpec::Delete { paths } => runner.delete(paths).await,
    };
    if result.is_ok() {
        runner.progress.total_entries =
            Some(runner.progress.completed_entries + runner.progress.skipped_entries);
        runner.progress.total_bytes = Some(runner.progress.transferred_bytes);
    }
    let cleanup_issues = runner.cleanup().await;
    FileCompletion {
        result,
        progress: runner.progress,
        cleanup_issues,
    }
}
