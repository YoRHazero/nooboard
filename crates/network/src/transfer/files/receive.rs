//! Ordered chunk verification and all-or-nothing publication.
use super::filesystem::{numbered_name, rename_exclusive, safe_name, staging};
use super::{CHUNK_BYTES, FileSpec, MAX_BATCH_BYTES, MAX_FILE_BYTES, MAX_FILES};
use crate::error::{Failure as Error, InternalResult as Result};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use tempfile::TempDir;
/// One receive transaction. Creates the receive directory if its parent exists.
/// Dropping it removes only this transaction's staging directory.
pub struct IncomingBatch {
    directory: TempDir,
    lease: Option<File>,
    root: PathBuf,
    specs: Vec<FileSpec>,
    names: Vec<String>,
    index: usize,
    offset: u64,
    current: Option<File>,
    digest: Sha256,
}
impl IncomingBatch {
    pub fn create(root: &Path, specs: Vec<FileSpec>) -> Result<Self> {
        if specs.is_empty()
            || specs.len() > MAX_FILES
            || specs.iter().any(|f| f.bytes > MAX_FILE_BYTES)
            || specs
                .iter()
                .try_fold(0u64, |n, f| n.checked_add(f.bytes))
                .is_none_or(|n| n > MAX_BATCH_BYTES)
        {
            return Err(Error::TooLarge);
        }
        // Create only the final directory: a missing download folder or mount must
        // fail instead of silently reconstructing its parents elsewhere.
        match fs::create_dir(root) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
        let root = root.canonicalize()?;
        if !root.is_dir() {
            return Err(Error::InvalidArgument("receive directory"));
        }
        let (directory, lease) = staging(&root, ".nooboard-receive-")?;
        let mut names: Vec<String> = Vec::new();
        for spec in &specs {
            let base = safe_name(&spec.name);
            let mut name = base.clone();
            let mut suffix = 1;
            while names
                .iter()
                .any(|n| n.to_lowercase() == name.to_lowercase())
            {
                name = numbered_name(&base, suffix);
                suffix += 1;
            }
            names.push(name);
        }
        let mut transaction = Self {
            directory,
            lease: Some(lease),
            root,
            specs,
            names,
            index: 0,
            offset: 0,
            current: None,
            digest: Sha256::new(),
        };
        transaction.advance()?;
        Ok(transaction)
    }
    fn advance(&mut self) -> Result<()> {
        while self.index < self.specs.len() {
            self.current = Some(
                OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(self.directory.path().join(&self.names[self.index]))?,
            );
            if self.specs[self.index].bytes != 0 {
                return Ok(());
            }
            self.finish_file()?;
        }
        Ok(())
    }
    fn finish_file(&mut self) -> Result<()> {
        if self.offset != self.specs[self.index].bytes
            || <[u8; 32]>::from(self.digest.clone().finalize()) != self.specs[self.index].sha256
        {
            return Err(Error::Integrity);
        }
        if let Some(file) = self.current.take() {
            file.sync_all()?;
        }
        self.index += 1;
        self.offset = 0;
        self.digest = Sha256::new();
        Ok(())
    }
    pub fn write(&mut self, index: usize, offset: u64, bytes: &[u8]) -> Result<()> {
        if index != self.index
            || index >= self.specs.len()
            || offset != self.offset
            || bytes.is_empty()
            || bytes.len() > CHUNK_BYTES
            || offset
                .checked_add(bytes.len() as u64)
                .is_none_or(|end| end > self.specs[index].bytes)
        {
            return Err(Error::Integrity);
        }
        self.current
            .as_mut()
            .ok_or(Error::Integrity)?
            .write_all(bytes)?;
        self.digest.update(bytes);
        self.offset += bytes.len() as u64;
        if self.offset == self.specs[index].bytes {
            self.finish_file()?;
            self.advance()?;
        }
        Ok(())
    }
    pub fn complete(&self) -> Result<()> {
        if self.index == self.specs.len() {
            Ok(())
        } else {
            Err(Error::Integrity)
        }
    }
    pub fn read_staged(&self, index: usize, limit: u64) -> Result<Vec<u8>> {
        self.complete()?;
        if self.specs.get(index).is_none_or(|s| s.bytes > limit) {
            return Err(Error::TooLarge);
        }
        Ok(fs::read(self.directory.path().join(&self.names[index]))?)
    }
    /// Caller has entered its non-cancellable commit phase. Existing user files are never replaced.
    pub fn commit(mut self) -> Result<Vec<PathBuf>> {
        self.complete()?;
        // Remove only the internal marker before publication. A markerless directory is never swept.
        fs::remove_file(self.directory.path().join(".nooboard-lease"))?;
        self.lease.take();
        if self.specs.len() == 1 {
            let source = self.directory.path().join(&self.names[0]);
            for suffix in 0..10_000 {
                let target = self.root.join(numbered_name(&self.names[0], suffix));
                match rename_exclusive(&source, &target) {
                    Ok(()) => return Ok(vec![target]),
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(e) => return Err(e.into()),
                }
            }
        } else {
            // Random batch name plus exclusive rename publishes the complete batch in one step.
            let suffix = self
                .directory
                .path()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .replace(".nooboard-receive-", "");
            for number in 0..10_000 {
                let target = self
                    .root
                    .join(numbered_name(&format!("nooboard-{suffix}"), number));
                match rename_exclusive(self.directory.path(), &target) {
                    Ok(()) => return Ok(self.names.iter().map(|name| target.join(name)).collect()),
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(e) => return Err(e.into()),
                }
            }
        }
        Err(Error::InvalidArgument("too many name collisions"))
    }
}
