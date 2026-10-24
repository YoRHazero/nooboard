use crate::{Error as Failure, IdentityOptions, Result, backend::Material};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

pub(super) fn load(profile: &str, options: &IdentityOptions) -> Result<Material> {
    // Coordinate creation across processes using the same profile. No key material enters the lock.
    #[cfg(not(unix))]
    let directory = std::env::temp_dir().join("nooboard-identity-locks");
    #[cfg(unix)]
    let directory = {
        use std::os::unix::fs::{DirBuilderExt, MetadataExt};
        // SAFETY: geteuid has no preconditions or side effects.
        let uid = unsafe { libc::geteuid() };
        let directory = std::env::temp_dir().join(format!("nooboard-identity-locks-{uid}"));
        match std::fs::DirBuilder::new().mode(0o700).create(&directory) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(Failure::Credentials),
        }
        let metadata = std::fs::symlink_metadata(&directory).map_err(|_| Failure::Credentials)?;
        if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o077 != 0 {
            return Err(Failure::Credentials);
        }
        directory
    };
    #[cfg(not(unix))]
    std::fs::create_dir_all(&directory).map_err(|_| Failure::Credentials)?;
    let mut file_options = std::fs::OpenOptions::new();
    file_options
        .read(true)
        .write(true)
        .create(true)
        .truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        file_options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let lock = file_options
        .open(directory.join(hex::encode(Sha256::digest(profile.as_bytes()))))
        .map_err(|_| Failure::Credentials)?;
    let deadline = std::time::Instant::now() + options.credential_lock_timeout;
    loop {
        match lock.try_lock() {
            Ok(()) => break,
            Err(std::fs::TryLockError::WouldBlock) => {
                if std::time::Instant::now() >= deadline {
                    return Err(Failure::Busy);
                }
                std::thread::sleep(options.credential_lock_poll_interval);
            }
            Err(std::fs::TryLockError::Error(_)) => return Err(Failure::Credentials),
        }
    }
    let entry =
        keyring::Entry::new("nooboard.identity.v1", profile).map_err(|_| Failure::Credentials)?;
    super::load_or_create(
        || match entry.get_secret() {
            Ok(bytes) => Ok(Some(Zeroizing::new(bytes))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(Failure::Credentials),
        },
        |secret| entry.set_secret(secret).map_err(|_| Failure::Credentials),
    )
}
