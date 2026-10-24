use crate::{IdentitySignatureScheme as Scheme, LocalIdentity};
use rustls::{
    SignatureAlgorithm, SignatureScheme,
    pki_types::CertificateDer,
    sign::{CertifiedKey, Signer, SigningKey},
};
use std::sync::Arc;

pub(super) fn certified(identity: &LocalIdentity) -> CertifiedKey {
    CertifiedKey::new(
        vec![CertificateDer::from(identity.certificate.clone())],
        Arc::new(Key(identity.clone())),
    )
}
#[derive(Debug)]
struct Key(LocalIdentity);
impl SigningKey for Key {
    fn choose_scheme(&self, offered: &[SignatureScheme]) -> Option<Box<dyn Signer>> {
        let scheme = self
            .0
            .schemes
            .iter()
            .copied()
            .find(|s| offered.contains(&wire(*s)))?;
        Some(Box::new(Signature {
            identity: self.0.clone(),
            scheme,
        }))
    }
    fn algorithm(&self) -> SignatureAlgorithm {
        match self.0.schemes[0] {
            Scheme::EcdsaP256Sha256 | Scheme::EcdsaP384Sha384 => SignatureAlgorithm::ECDSA,
            Scheme::Ed25519 => SignatureAlgorithm::ED25519,
            Scheme::RsaPssSha256 | Scheme::RsaPssSha384 | Scheme::RsaPssSha512 => {
                SignatureAlgorithm::RSA
            }
        }
    }
}
#[derive(Debug)]
struct Signature {
    identity: LocalIdentity,
    scheme: Scheme,
}
impl Signer for Signature {
    fn sign(&self, message: &[u8]) -> std::result::Result<Vec<u8>, rustls::Error> {
        self.identity
            .signer
            .sign(self.scheme, message)
            .map_err(|_| rustls::Error::General("local identity signer unavailable".into()))
    }
    fn scheme(&self) -> SignatureScheme {
        wire(self.scheme)
    }
}
fn wire(scheme: Scheme) -> SignatureScheme {
    match scheme {
        Scheme::EcdsaP256Sha256 => SignatureScheme::ECDSA_NISTP256_SHA256,
        Scheme::EcdsaP384Sha384 => SignatureScheme::ECDSA_NISTP384_SHA384,
        Scheme::Ed25519 => SignatureScheme::ED25519,
        Scheme::RsaPssSha256 => SignatureScheme::RSA_PSS_SHA256,
        Scheme::RsaPssSha384 => SignatureScheme::RSA_PSS_SHA384,
        Scheme::RsaPssSha512 => SignatureScheme::RSA_PSS_SHA512,
    }
}
