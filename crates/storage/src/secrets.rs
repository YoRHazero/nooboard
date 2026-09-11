//! Native credential storage; unsupported platforms never silently use a mock.
#[derive(Debug, thiserror::Error)]
#[error("native credential store operation failed")]
pub struct SecretError;
pub struct SecretStore {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    entry: keyring::Entry,
}
impl SecretStore {
    pub fn new(profile: &str) -> Result<Self, SecretError> {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            Ok(Self {
                entry: keyring::Entry::new("nooboard.identity.v1", profile)
                    .map_err(|_| SecretError)?,
            })
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = profile;
            Err(SecretError)
        }
    }
    pub fn get(&self) -> Result<Option<Vec<u8>>, SecretError> {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            match self.entry.get_secret() {
                Ok(secret) => Ok(Some(secret)),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(_) => Err(SecretError),
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            Err(SecretError)
        }
    }
    pub fn set(&self, secret: &[u8]) -> Result<(), SecretError> {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            self.entry.set_secret(secret).map_err(|_| SecretError)
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = secret;
            Err(SecretError)
        }
    }
}
