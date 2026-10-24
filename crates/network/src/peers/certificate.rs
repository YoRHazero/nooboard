use crate::error::InternalResult as Result;
use rustls::pki_types::CertificateDer;
use sha2::{Digest, Sha256};

pub fn fingerprint(certificate: &[u8]) -> String {
    hex::encode(Sha256::digest(certificate))
}

/// Stable device identity: SHA-256 of DER SubjectPublicKeyInfo, not the certificate.
pub fn noob_id(certificate: &[u8]) -> Result<String> {
    let der = CertificateDer::from(certificate);
    let parsed = rustls::server::ParsedCertificate::try_from(&der)?;
    Ok(hex::encode(Sha256::digest(
        parsed.subject_public_key_info(),
    )))
}
