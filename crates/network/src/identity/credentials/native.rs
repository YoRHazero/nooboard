use crate::{
    error::{Failure, InternalResult as Result},
    identity::material::Identity,
};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

pub(super) fn load(profile: &str) -> Result<Identity> {
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
            Err(e) => return Err(e.into()),
        }
        let metadata = std::fs::symlink_metadata(&directory)?;
        if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o077 != 0 {
            return Err(Failure::Credentials);
        }
        directory
    };
    #[cfg(not(unix))]
    std::fs::create_dir_all(&directory)?;
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let lock = options.open(directory.join(hex::encode(Sha256::digest(profile.as_bytes()))))?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        match lock.try_lock() {
            Ok(()) => break,
            Err(std::fs::TryLockError::WouldBlock) => {
                if std::time::Instant::now() >= deadline {
                    return Err(Failure::Busy);
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            Err(std::fs::TryLockError::Error(error)) => return Err(error.into()),
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
