use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub enum IdentityConfig {
    /// Reads the existing nooboard.identity.v1 entry; creates only when truly absent.
    System { profile: String },
    /// Explicitly temporary. Never used as a fallback for a failed credential read.
    Ephemeral,
}

/// TLS 1.3 signature schemes; messages are signed before hashing, with the named scheme.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignatureScheme {
    EcdsaP256Sha256,
    EcdsaP384Sha384,
    Ed25519,
    RsaPssSha256,
    RsaPssSha384,
    RsaPssSha512,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicIdentity {
    pub id: String,
    pub fingerprint: String,
    pub certificate: Vec<u8>,
    pub signature_schemes: Vec<SignatureScheme>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceState {
    Running,
    Stopping,
    Stopped,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdentityStatus {
    pub state: ServiceState,
    pub identity: PublicIdentity,
}

/// Caller-supplied domain and binding. Core binds its config ID, field and format here.
/// This contains public metadata, never passwords or other secret input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecretContext {
    pub purpose: String,
    pub associated_data: Vec<u8>,
}

/// Version 1: HKDF-SHA256 + XChaCha20Poly1305, with a fresh random nonce per seal.
/// Application configuration and persistence remain the caller's responsibility.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SealedSecret {
    pub version: u32,
    pub nonce: [u8; 24],
    pub ciphertext: Vec<u8>,
}
impl std::fmt::Debug for SealedSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SealedSecret")
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}
