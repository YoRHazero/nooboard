use super::Password;
use crate::{Error, Result};

#[cfg(feature = "native-credentials")]
pub(super) fn load(namespace: &str, key: &str) -> Result<Password> {
    keyring::Entry::new(namespace, key)
        .and_then(|e| e.get_password())
        .map(Password::new)
        .map_err(|_| Error::Credentials)
}
#[cfg(feature = "native-credentials")]
pub(super) fn save(namespace: &str, key: &str, password: &Password) -> Result<()> {
    keyring::Entry::new(namespace, key)
        .and_then(|e| e.set_password(password.expose()))
        .map_err(|_| Error::Credentials)
}
#[cfg(feature = "native-credentials")]
pub(super) fn delete(namespace: &str, key: &str) -> Result<()> {
    match keyring::Entry::new(namespace, key).and_then(|e| e.delete_credential()) {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err(Error::Credentials),
    }
}
#[cfg(not(feature = "native-credentials"))]
pub(super) fn load(_: &str, _: &str) -> Result<Password> {
    Err(Error::Credentials)
}
#[cfg(not(feature = "native-credentials"))]
pub(super) fn save(_: &str, _: &str, _: &Password) -> Result<()> {
    Err(Error::Credentials)
}
#[cfg(not(feature = "native-credentials"))]
pub(super) fn delete(_: &str, _: &str) -> Result<()> {
    Err(Error::Credentials)
}
