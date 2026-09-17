//! Bounded disk staging and publication. No network or clipboard dependencies.
use crate::{Error, Result};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
use tempfile::TempDir;

pub const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub const MAX_BATCH_BYTES: u64 = 8 * 1024 * 1024 * 1024;
pub const MAX_FILES: usize = 256;
pub const CHUNK_BYTES: usize = 256 * 1024;

#[derive(Clone, Debug)]
pub struct FileSpec {
    pub name: String,
    pub bytes: u64,
    pub sha256: [u8; 32],
}

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
fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}
fn open_source(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x00200000);
    }
    Ok(options.open(path)?)
}
pub fn check_cancelled(cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Acquire) {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}

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

const STAGING_MARKER: &[u8] = b"nooboard-transfer-staging-v1\n";
fn staging(root: &Path, prefix: &str) -> Result<(TempDir, File)> {
    cleanup_staging(root, prefix);
    let directory = tempfile::Builder::new().prefix(prefix).tempdir_in(root)?;
    let mut lease = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(directory.path().join(".nooboard-lease"))?;
    lease
        .try_lock()
        .map_err(|_| Error::InvalidArgument("staging lease"))?;
    lease.write_all(STAGING_MARKER)?;
    Ok((directory, lease))
}
fn cleanup_staging(root: &Path, prefix: &str) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.take(4096).flatten() {
        if !entry.file_name().to_string_lossy().starts_with(prefix)
            || !entry.file_type().is_ok_and(|t| t.is_dir())
        {
            continue;
        }
        let marker = entry.path().join(".nooboard-lease");
        let Ok(metadata) = fs::symlink_metadata(&marker) else {
            continue;
        };
        if !metadata.is_file()
            || is_link(&metadata)
            || metadata.len() != STAGING_MARKER.len() as u64
        {
            continue;
        }
        let Ok(mut lease) = open_source(&marker) else {
            continue;
        };
        if lease.try_lock().is_err() {
            continue;
        }
        let mut bytes = Vec::new();
        if lease.read_to_end(&mut bytes).is_ok() && bytes == STAGING_MARKER {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

fn numbered_name(name: &str, suffix: usize) -> String {
    if suffix == 0 {
        return name.into();
    }
    let path = Path::new(name);
    match (path.file_stem(), path.extension()) {
        (Some(stem), Some(ext)) => format!(
            "{} ({suffix}).{}",
            stem.to_string_lossy(),
            ext.to_string_lossy()
        ),
        _ => format!("{name} ({suffix})"),
    }
}
pub fn safe_name(name: &str) -> String {
    let mut result = String::new();
    for c in name.chars() {
        let c = if c.is_control() || "<>:\"/\\|?*".contains(c) {
            '_'
        } else {
            c
        };
        if result.len() + c.len_utf8() > 180 {
            break;
        }
        result.push(c);
    }
    let mut result = result.trim_matches([' ', '.']).to_owned();
    if result.is_empty() {
        result = "file".into();
    }
    let stem = result.split('.').next().unwrap().to_ascii_uppercase();
    if ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].contains(&stem.as_str())
        || ["COM", "LPT"].iter().any(|p| {
            stem.strip_prefix(p).is_some_and(|n| {
                matches!(
                    n,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
        })
    {
        result.insert(0, '_');
    }
    result
}

#[cfg(unix)]
fn rename_exclusive(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let from = CString::new(from.as_os_str().as_bytes())?;
    let to = CString::new(to.as_os_str().as_bytes())?;
    // SAFETY: both pointers are valid NUL-terminated paths; exclusive rename never replaces a destination.
    let status = unsafe {
        #[cfg(target_os = "macos")]
        {
            libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL)
        }
        #[cfg(target_os = "linux")]
        {
            libc::renameat2(
                libc::AT_FDCWD,
                from.as_ptr(),
                libc::AT_FDCWD,
                to.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            return Err(std::io::Error::from(std::io::ErrorKind::Unsupported));
        }
    };
    if status == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}
#[cfg(windows)]
fn rename_exclusive(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    let from: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: valid terminated paths; zero flags disallow replacing an existing destination.
    if unsafe {
        windows_sys::Win32::Storage::FileSystem::MoveFileExW(from.as_ptr(), to.as_ptr(), 0)
    } != 0
    {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crash_cleanup_keeps_live_transactions_and_user_directories() {
        let root = tempfile::tempdir().unwrap();
        let (live, lease) = staging(root.path(), ".nooboard-receive-").unwrap();
        let (orphan, old_lease) = staging(root.path(), ".nooboard-receive-").unwrap();
        let orphan = orphan.keep();
        drop(old_lease);
        let user = root.path().join(".nooboard-receive-user");
        fs::create_dir(&user).unwrap();
        cleanup_staging(root.path(), ".nooboard-receive-");
        assert!(live.path().is_dir());
        assert!(user.is_dir());
        assert!(!orphan.exists());
        drop(lease);
    }
    fn spec(name: &str, bytes: &[u8]) -> FileSpec {
        FileSpec {
            name: name.into(),
            bytes: bytes.len() as u64,
            sha256: Sha256::digest(bytes).into(),
        }
    }
    #[test]
    fn receive_directory_is_created_for_first_transfer_and_reused() {
        let downloads = tempfile::tempdir().unwrap();
        let root = downloads.path().join("Nooboard");
        assert!(!root.exists());
        for content in [b"first".as_slice(), b"second".as_slice()] {
            let mut batch = IncomingBatch::create(&root, vec![spec("file", content)]).unwrap();
            batch.write(0, 0, content).unwrap();
            let paths = batch.commit().unwrap();
            assert_eq!(
                paths[0].parent(),
                Some(root.canonicalize().unwrap().as_path())
            );
            assert_eq!(fs::read(&paths[0]).unwrap(), content);
        }
        assert_eq!(fs::read(root.join("file")).unwrap(), b"first");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 2);
    }
    #[test]
    fn unavailable_receive_directory_does_not_recreate_parents_or_replace_files() {
        let downloads = tempfile::tempdir().unwrap();
        let root = downloads.path().join("Nooboard");
        fs::write(&root, b"existing file").unwrap();
        assert!(IncomingBatch::create(&root, vec![spec("file", b"data")]).is_err());
        assert_eq!(fs::read(&root).unwrap(), b"existing file");
        let missing = downloads.path().join("unavailable mount");
        assert!(
            IncomingBatch::create(&missing.join("Nooboard"), vec![spec("file", b"data")]).is_err()
        );
        assert!(!missing.exists());
    }
    #[test]
    fn verified_batch_publishes_together_and_handles_case_collisions_and_empty_files() {
        let root = tempfile::tempdir().unwrap();
        let mut batch = IncomingBatch::create(
            root.path(),
            vec![
                spec("../../A.txt", b"abc"),
                spec("../../a.txt", b""),
                spec("CON", b"z"),
            ],
        )
        .unwrap();
        assert!(batch.complete().is_err());
        batch.write(0, 0, b"abc").unwrap();
        batch.write(2, 0, b"z").unwrap();
        let paths = batch.commit().unwrap();
        assert_eq!(fs::read(&paths[0]).unwrap(), b"abc");
        assert!(fs::read(&paths[1]).unwrap().is_empty());
        assert_ne!(
            paths[0].to_string_lossy().to_lowercase(),
            paths[1].to_string_lossy().to_lowercase()
        );
        assert!(
            paths
                .iter()
                .all(|p| p.starts_with(root.path().canonicalize().unwrap())
                    && p.parent() == paths[0].parent())
        );
    }
    #[test]
    fn checksum_failure_and_drop_leave_no_formal_files_and_existing_file_survives() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("a.txt"), b"original").unwrap();
        {
            let mut b = IncomingBatch::create(root.path(), vec![spec("a.txt", b"new")]).unwrap();
            assert!(b.write(0, 0, b"bad").is_err());
        }
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
        let mut b = IncomingBatch::create(root.path(), vec![spec("a.txt", b"new")]).unwrap();
        b.write(0, 0, b"new").unwrap();
        let paths = b.commit().unwrap();
        assert_ne!(paths[0], root.path().join("a.txt"));
        assert_eq!(fs::read(root.path().join("a.txt")).unwrap(), b"original");
    }
    #[test]
    fn invalid_offsets_and_preparation_cancellation_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        let mut b = IncomingBatch::create(root.path(), vec![spec("a", b"abc")]).unwrap();
        assert!(b.write(0, 1, b"a").is_err());
        assert!(b.write(0, 0, b"abcd").is_err());
        let path = root.path().join("source");
        fs::write(&path, b"data").unwrap();
        assert!(matches!(
            PreparedBatch::from_paths(&[path], &AtomicBool::new(true), |_, _| {}),
            Err(Error::Cancelled)
        ));
    }
    #[test]
    fn changing_source_during_preparation_fails_the_whole_batch() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("changing.bin");
        fs::write(&path, vec![7; CHUNK_BYTES * 2]).unwrap();
        let mut changed = false;
        let result = PreparedBatch::from_paths(
            std::slice::from_ref(&path),
            &AtomicBool::new(false),
            |done, _| {
                if done > 0 && !changed {
                    OpenOptions::new()
                        .write(true)
                        .open(&path)
                        .unwrap()
                        .set_len(1)
                        .unwrap();
                    changed = true;
                }
            },
        );
        assert!(matches!(result, Err(Error::SourceChanged)));
    }
}
