//! Core adapts Identity capabilities to consumer-owned ports; services never depend on each other.
use nooboard_identity::{Identity, SignatureScheme as IdentityScheme};
use nooboard_network::{
    IdentitySignatureScheme as NetworkScheme, IdentitySigner, IdentitySigningError, LocalIdentity,
    NetworkDependencies,
};
use std::sync::Arc;

pub(crate) fn network_dependencies(identity: Identity) -> crate::Result<NetworkDependencies> {
    let certificate = identity.public_identity().certificate;
    Ok(NetworkDependencies {
        identity: LocalIdentity::new(certificate, Arc::new(NetworkSigner(identity)))?,
    })
}
struct NetworkSigner(Identity);
impl IdentitySigner for NetworkSigner {
    fn supported_schemes(&self) -> Vec<NetworkScheme> {
        self.0
            .public_identity()
            .signature_schemes
            .into_iter()
            .map(|scheme| match scheme {
                IdentityScheme::EcdsaP256Sha256 => NetworkScheme::EcdsaP256Sha256,
                IdentityScheme::EcdsaP384Sha384 => NetworkScheme::EcdsaP384Sha384,
                IdentityScheme::Ed25519 => NetworkScheme::Ed25519,
                IdentityScheme::RsaPssSha256 => NetworkScheme::RsaPssSha256,
                IdentityScheme::RsaPssSha384 => NetworkScheme::RsaPssSha384,
                IdentityScheme::RsaPssSha512 => NetworkScheme::RsaPssSha512,
            })
            .collect()
    }
    fn sign(
        &self,
        scheme: NetworkScheme,
        message: &[u8],
    ) -> std::result::Result<Vec<u8>, IdentitySigningError> {
        let scheme = match scheme {
            NetworkScheme::EcdsaP256Sha256 => IdentityScheme::EcdsaP256Sha256,
            NetworkScheme::EcdsaP384Sha384 => IdentityScheme::EcdsaP384Sha384,
            NetworkScheme::Ed25519 => IdentityScheme::Ed25519,
            NetworkScheme::RsaPssSha256 => IdentityScheme::RsaPssSha256,
            NetworkScheme::RsaPssSha384 => IdentityScheme::RsaPssSha384,
            NetworkScheme::RsaPssSha512 => IdentityScheme::RsaPssSha512,
        };
        self.0
            .sign_blocking(scheme, message)
            .map_err(|_| IdentitySigningError)
    }
}
