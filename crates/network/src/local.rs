use crate::error::{Failure, InternalResult};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentitySignatureScheme {
    EcdsaP256Sha256,
    EcdsaP384Sha384,
    Ed25519,
    RsaPssSha256,
    RsaPssSha384,
    RsaPssSha512,
}
#[derive(Clone, Copy, Debug, thiserror::Error)]
#[error("local identity signer is unavailable")]
pub struct IdentitySigningError;

/// A synchronous, bounded signing capability supplied by the application composer.
/// Network never obtains the application private key or chooses its storage backend.
pub trait IdentitySigner: Send + Sync {
    fn supported_schemes(&self) -> Vec<IdentitySignatureScheme>;
    fn sign(
        &self,
        scheme: IdentitySignatureScheme,
        message: &[u8],
    ) -> std::result::Result<Vec<u8>, IdentitySigningError>;
}
#[derive(Clone)]
pub struct LocalIdentity {
    pub(crate) certificate: Vec<u8>,
    pub(crate) schemes: Vec<IdentitySignatureScheme>,
    pub(crate) signer: Arc<dyn IdentitySigner>,
}
impl std::fmt::Debug for LocalIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalIdentity")
            .field("schemes", &self.schemes)
            .finish_non_exhaustive()
    }
}
impl LocalIdentity {
    pub fn new(certificate: Vec<u8>, signer: Arc<dyn IdentitySigner>) -> crate::Result<Self> {
        if certificate.len() > 8192 {
            return Err(Failure::Identity.into());
        }
        crate::peers::certificate::noob_id(&certificate)?;
        let schemes = signer.supported_schemes();
        if schemes.is_empty() {
            return Err(Failure::Identity.into());
        }
        Ok(Self {
            certificate,
            schemes,
            signer,
        })
    }
    pub fn certificate(&self) -> &[u8] {
        &self.certificate
    }
    pub(crate) fn noob_id(&self) -> InternalResult<String> {
        crate::peers::certificate::noob_id(&self.certificate)
    }
    pub(crate) fn fingerprint(&self) -> String {
        crate::peers::certificate::fingerprint(&self.certificate)
    }
}
