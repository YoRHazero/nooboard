use zeroize::Zeroizing;
/// Sensitive runtime text: deliberately not serializable and always redacted in Debug.
#[derive(Clone)]
pub struct SecretString(Zeroizing<String>);
impl SecretString {
    pub fn new(value: impl Into<String>) -> Self {
        Self(Zeroizing::new(value.into()))
    }
    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Debug for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretString([REDACTED])")
    }
}
/// Unencrypted OpenSSH/PKCS#8 PEM private-key material supplied by the caller.
pub struct SecretBytes(Zeroizing<Vec<u8>>);
impl SecretBytes {
    pub fn new(value: Vec<u8>) -> Self {
        Self(Zeroizing::new(value))
    }
    pub(crate) fn expose(&self) -> &[u8] {
        &self.0
    }
}
impl std::fmt::Debug for SecretBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretBytes([REDACTED])")
    }
}
