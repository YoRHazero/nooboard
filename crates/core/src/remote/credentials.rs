//! Policy depends on a vault port; platform drivers remain in the SSH crate.
use nooboard_ssh::{CredentialStore, Password, Result};
use std::{future::Future, pin::Pin, sync::Arc};
type Operation<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;
pub(crate) trait Vault: Send + Sync {
    fn load(&self, key: String) -> Operation<'_, Password>;
    fn save(&self, key: String, password: Password) -> Operation<'_, ()>;
    fn delete(&self, key: String) -> Operation<'_, ()>;
}
impl Vault for CredentialStore {
    fn load(&self, key: String) -> Operation<'_, Password> {
        Box::pin(self.load(key))
    }
    fn save(&self, key: String, password: Password) -> Operation<'_, ()> {
        Box::pin(self.save(key, password))
    }
    fn delete(&self, key: String) -> Operation<'_, ()> {
        Box::pin(self.delete(key))
    }
}
pub(crate) fn store(profile: &str) -> Arc<dyn Vault> {
    #[cfg(not(test))]
    {
        Arc::new(CredentialStore::new(format!("nooboard.ssh.{profile}")))
    }
    #[cfg(test)]
    {
        let _ = profile;
        Arc::new(Memory::default())
    }
}
#[cfg(test)]
#[derive(Default)]
struct Memory(std::sync::Mutex<std::collections::HashMap<String, Password>>);
#[cfg(test)]
impl Vault for Memory {
    fn load(&self, key: String) -> Operation<'_, Password> {
        Box::pin(async move {
            self.0
                .lock()
                .unwrap()
                .get(&key)
                .map(Password::duplicate)
                .ok_or(nooboard_ssh::Error::Credentials)
        })
    }
    fn save(&self, key: String, password: Password) -> Operation<'_, ()> {
        Box::pin(async move {
            self.0.lock().unwrap().insert(key, password);
            Ok(())
        })
    }
    fn delete(&self, key: String) -> Operation<'_, ()> {
        Box::pin(async move {
            self.0.lock().unwrap().remove(&key);
            Ok(())
        })
    }
}
