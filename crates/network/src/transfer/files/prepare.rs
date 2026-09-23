//! Snapshot sources once so every target reads the same bytes.
use super::filesystem::{is_link, open_source, safe_name, staging};
use super::{CHUNK_BYTES, FileSpec, MAX_BATCH_BYTES, MAX_FILE_BYTES, MAX_FILES, check_cancelled};
use crate::error::{Failure as Error, InternalResult as Result};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::PathBuf,
    sync::atomic::AtomicBool,
};
use tempfile::TempDir;
pub struct PreparedBatch {
    directory: TempDir,
    _lease: File,
    pub files: Vec<FileSpec>,
}
impl PreparedBatch {
    pub fn open(&self, index: usize) -> Result<File> {
        if index >= self.files.len() {
            return Err(Error::InvalidArgument("file index"));
        }
        Ok(File::open(self.directory.path().join(index.to_string()))?)
    }
    pub fn bytes(&self) -> u64 {
        self.files.iter().map(|f| f.bytes).sum()
    }
    pub fn from_bytes(name: &str, bytes: &[u8]) -> Result<Self> {
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err(Error::TooLarge);
        }
        let (directory, lease) = staging(&std::env::temp_dir(), "nooboard-send-")?;
        let mut file = File::create(directory.path().join("0"))?;
        file.write_all(bytes)?;
        Ok(Self {
            directory,
            _lease: lease,
            files: vec![FileSpec {
                name: safe_name(name),
                bytes: bytes.len() as u64,
                sha256: Sha256::digest(bytes).into(),
            }],
        })
    }
    /// All destinations read these same immutable staged bytes. Cancellation is checked per chunk.
    pub fn from_paths(
        paths: &[PathBuf],
        cancelled: &AtomicBool,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<Self> {
        if paths.is_empty() || paths.len() > MAX_FILES {
            return Err(Error::TooLarge);
        }
        let mut sources = Vec::with_capacity(paths.len());
        let mut total = 0u64;
        for path in paths {
            check_cancelled(cancelled)?;
            let metadata = fs::symlink_metadata(path)?;
            if !metadata.is_file() || is_link(&metadata) {
                return Err(Error::InvalidArgument("ordinary local files only"));
            }
            let file = open_source(path)?;
            let metadata = file.metadata()?;
            if !metadata.is_file() || is_link(&metadata) {
                return Err(Error::InvalidArgument("ordinary local files only"));
            }
            total = total.checked_add(metadata.len()).ok_or(Error::TooLarge)?;
            if metadata.len() > MAX_FILE_BYTES || total > MAX_BATCH_BYTES {
                return Err(Error::TooLarge);
            }
            sources.push((file, metadata));
        }
        let (directory, lease) = staging(&std::env::temp_dir(), "nooboard-send-")?;
        let mut files = Vec::new();
        let mut buffer = vec![0; CHUNK_BYTES];
        let mut done = 0;
        progress(done, total);
        for (index, (mut source, before)) in sources.into_iter().enumerate() {
            let mut target = File::create(directory.path().join(index.to_string()))?;
            let mut digest = Sha256::new();
            let mut size = 0;
            loop {
                check_cancelled(cancelled)?;
                let count = source.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                size += count as u64;
                if size > before.len() {
                    return Err(Error::SourceChanged);
                }
                target.write_all(&buffer[..count])?;
                digest.update(&buffer[..count]);
                done += count as u64;
                progress(done, total);
            }
            let after = source.metadata()?;
            let current = fs::symlink_metadata(&paths[index])?;
            if size != before.len()
                || !same_file(&before, &after)
                || !same_file(&before, &current)
                || is_link(&current)
            {
                return Err(Error::SourceChanged);
            }
            files.push(FileSpec {
                name: safe_name(
                    &paths[index]
                        .file_name()
                        .ok_or(Error::InvalidArgument("file name"))?
                        .to_string_lossy(),
                ),
                bytes: size,
                sha256: digest.finalize().into(),
            });
        }
        Ok(Self {
            directory,
            _lease: lease,
            files,
        })
    }
}

fn same_file(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if a.dev() != b.dev() || a.ino() != b.ino() {
            return false;
        }
    }
    a.len() == b.len() && a.modified().ok() == b.modified().ok()
}
