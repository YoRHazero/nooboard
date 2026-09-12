//! Native credential storage; unsupported platforms never silently use a mock.
#[derive(Debug, thiserror::Error)]
#[error("native credential store operation failed")]
pub struct SecretError;
pub struct SecretStore {
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    entry: keyring::Entry,
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires an isolated, unlocked Secret Service session"]
    fn native_secret_service_roundtrip() {
        let profile = format!(
            "test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let store = SecretStore::new(&profile).unwrap();
        struct Cleanup<'a>(&'a SecretStore);
        impl Drop for Cleanup<'_> {
            fn drop(&mut self) {
                let _ = self.0.entry.delete_credential();
            }
        }
        let _cleanup = Cleanup(&store);
        assert!(store.get().unwrap().is_none());
        store.set(b"nooboard diagnostic credential").unwrap();
        assert_eq!(
            SecretStore::new(&profile).unwrap().get().unwrap().unwrap(),
            b"nooboard diagnostic credential"
        );
        store.set(b"replacement").unwrap();
        assert_eq!(store.get().unwrap().unwrap(), b"replacement");
        store.entry.delete_credential().unwrap();
        assert!(store.get().unwrap().is_none());
    }
}
impl SecretStore {
    pub fn new(profile: &str) -> Result<Self, SecretError> {
        #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
        {
            Ok(Self {
                entry: keyring::Entry::new("nooboard.identity.v1", profile)
                    .map_err(|_| SecretError)?,
            })
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            let _ = profile;
            Err(SecretError)
        }
    }
    pub fn get(&self) -> Result<Option<Vec<u8>>, SecretError> {
        #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
        {
            match self.entry.get_secret() {
                Ok(secret) => Ok(Some(secret)),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(_) => Err(SecretError),
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            Err(SecretError)
        }
    }
    pub fn set(&self, secret: &[u8]) -> Result<(), SecretError> {
        #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
        {
            self.entry.set_secret(secret).map_err(|_| SecretError)
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            let _ = secret;
            Err(SecretError)
        }
    }
}
