mod native;
use std::fmt;
use zeroize::Zeroizing;

/// Deliberately neither serializable nor printable. Library authentication may make
/// short-lived copies internally; this wrapper clears the application's own copy.
pub struct Password(Zeroizing<String>);
impl Password {
    pub fn new(value: String) -> Self {
        Self(Zeroizing::new(value))
    }
    /// Explicit short-lived copy for an application that saves to its vault only
    /// after authentication. Both copies are zeroized when dropped.
    pub fn duplicate(&self) -> Self {
        Self::new(self.0.to_string())
    }
    pub(crate) fn expose(&self) -> &str {
        self.0.as_str()
    }
}
impl fmt::Debug for Password {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Password([redacted])")
    }
}

/// Namespace must be stable for this application profile. Eligibility to use a
/// saved credential belongs to core and is separate from vault presence.
#[derive(Clone)]
pub struct CredentialStore {
    namespace: String,
}
impl CredentialStore {
    pub fn new(namespace: String) -> Self {
        Self { namespace }
    }
    pub async fn load(&self, key: String) -> crate::Result<Password> {
        let namespace = self.namespace.clone();
        tokio::task::spawn_blocking(move || native::load(&namespace, &key))
            .await
            .map_err(|_| crate::Error::Credentials)?
    }
    pub async fn save(&self, key: String, password: Password) -> crate::Result<()> {
        let namespace = self.namespace.clone();
        tokio::task::spawn_blocking(move || native::save(&namespace, &key, &password))
            .await
            .map_err(|_| crate::Error::Credentials)?
    }
    pub async fn delete(&self, key: String) -> crate::Result<()> {
        let namespace = self.namespace.clone();
        tokio::task::spawn_blocking(move || native::delete(&namespace, &key))
            .await
            .map_err(|_| crate::Error::Credentials)?
    }
}
