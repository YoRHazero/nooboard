use super::model::{DirectoryEntry, FileKind};
use crate::{
    sftp::client::{Client, eof},
    *,
};
use std::{collections::VecDeque, sync::Arc};

/// Only unconsumed entries from the last protocol response live here. Delivered
/// entries and presentation state belong to the caller's workspace cache.
pub(super) struct Reader {
    client: Arc<Client>,
    handle: String,
    path: RemotePath,
    pending: VecDeque<DirectoryEntry>,
    complete: bool,
}
impl Reader {
    pub async fn open(client: Arc<Client>, path: RemotePath) -> Result<Self> {
        let handle = match client.raw.opendir(path.as_str()).await {
            Ok(handle) => handle.handle,
            Err(error) => {
                if !matches!(error, russh_sftp::client::error::Error::Status(_)) {
                    // Without the OPEN reply there is no safe handle to CLOSE.
                    client.shutdown().await;
                }
                return Err(error.into());
            }
        };
        Ok(Self {
            client,
            handle,
            path,
            pending: VecDeque::new(),
            complete: false,
        })
    }
    pub async fn next(
        &mut self,
        limit: usize,
        cancelled: impl Fn() -> bool,
    ) -> Result<(Vec<DirectoryEntry>, bool)> {
        let mut entries = Vec::with_capacity(limit);
        // Never prefetch to discover EOF after satisfying the current demand.
        let mut empty_packets = 0;
        while entries.len() < limit {
            if cancelled() {
                return Err(Error::Cancelled);
            }
            if let Some(entry) = self.pending.pop_front() {
                entries.push(entry);
                continue;
            }
            if self.complete {
                break;
            }
            match self.client.raw.readdir(self.handle.clone()).await {
                Ok(response) => {
                    for file in response.files {
                        if file.filename == "." || file.filename == ".." {
                            continue;
                        }
                        let path = self.path.child(&file.filename)?;
                        let kind = match file.attrs.permissions.map(|p| p & 0o170000) {
                            Some(0o040000) => FileKind::Directory,
                            Some(0o100000) => FileKind::File,
                            Some(0o120000) => FileKind::Symlink,
                            Some(_) => FileKind::Other,
                            None => FileKind::Unknown,
                        };
                        self.pending.push_back(DirectoryEntry {
                            name: file.filename,
                            path,
                            kind,
                            size: file.attrs.size,
                            modified: file.attrs.mtime,
                        });
                    }
                    if self.pending.is_empty() {
                        empty_packets += 1;
                        if empty_packets > 16 {
                            return Err(Error::Protocol(
                                "directory returned repeated empty batches".into(),
                            ));
                        }
                    } else {
                        empty_packets = 0;
                    }
                }
                Err(e) if eof(&e) => self.complete = true,
                Err(e) => return Err(e.into()),
            }
        }
        Ok((entries, self.complete && self.pending.is_empty()))
    }
    pub async fn close(self) {
        let _ = self.client.raw.close(self.handle).await;
    }
}
