use crate::{Error, Result};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use sha2::{Digest, Sha256};

pub fn fingerprint(certificate: &[u8]) -> String {
    hex::encode(Sha256::digest(certificate))
}

/// Private key material is never included in Debug or written to configuration files.
pub struct Identity {
    pub(crate) cert: CertificateDer<'static>,
    pub(crate) key: PrivateKeyDer<'static>,
}
impl Identity {
    pub fn generate() -> Result<Self> {
        let generated = rcgen::generate_simple_self_signed(vec!["nooboard.local".into()])
            .map_err(|_| Error::Identity)?;
        Ok(Self {
            cert: generated.cert.der().clone(),
            key: PrivatePkcs8KeyDer::from(generated.signing_key.serialize_der()).into(),
        })
    }
    pub fn certificate(&self) -> &[u8] {
        self.cert.as_ref()
    }
    pub fn fingerprint(&self) -> String {
        fingerprint(self.certificate())
    }
    /// Binary material intended exclusively for the OS credential store.
    pub fn export_secret(&self) -> Vec<u8> {
        let mut secret = (self.cert.len() as u32).to_be_bytes().to_vec();
        secret.extend_from_slice(&self.cert);
        secret.extend_from_slice(self.key.secret_der());
        secret
    }
    pub fn from_secret(secret: &[u8]) -> Result<Self> {
        if secret.len() < 5 || secret.len() > 8192 {
            return Err(Error::Identity);
        }
        let size =
            u32::from_be_bytes(secret[..4].try_into().map_err(|_| Error::Identity)?) as usize;
        if size == 0 || size >= secret.len() - 4 {
            return Err(Error::Identity);
        }
        let identity = Self {
            cert: CertificateDer::from(secret[4..4 + size].to_vec()),
            key: PrivatePkcs8KeyDer::from(secret[4 + size..].to_vec()).into(),
        };
        // The standard rustls builder parses the key and checks it against the certificate.
        rustls::ServerConfig::builder_with_provider(std::sync::Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .with_no_client_auth()
        .with_single_cert(vec![identity.cert.clone()], identity.key.clone_key())?;
        Ok(identity)
    }
}
