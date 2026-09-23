//! Platform filesystem operations, safe names and leased temporary directories.
use crate::error::{Failure as Error, InternalResult as Result};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
};
use tempfile::TempDir;
pub(super) fn is_link(metadata: &fs::Metadata) -> bool {
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
pub(super) fn open_source(path: &Path) -> Result<File> {
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
const STAGING_MARKER: &[u8] = b"nooboard-transfer-staging-v1\n";
pub(super) fn staging(root: &Path, prefix: &str) -> Result<(TempDir, File)> {
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
pub(super) fn cleanup_staging(root: &Path, prefix: &str) {
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

pub(super) fn numbered_name(name: &str, suffix: usize) -> String {
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
pub(super) fn safe_name(name: &str) -> String {
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
pub(super) fn rename_exclusive(from: &Path, to: &Path) -> std::io::Result<()> {
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
pub(super) fn rename_exclusive(from: &Path, to: &Path) -> std::io::Result<()> {
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
