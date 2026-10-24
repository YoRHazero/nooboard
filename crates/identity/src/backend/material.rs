use crate::{Error, PublicIdentity, Result, SignatureScheme};
use rustls::{
    pki_types::{CertificateDer, PrivatePkcs8KeyDer},
    sign::{CertifiedKey, SigningKey},
};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use zeroize::Zeroizing;

pub(crate) struct Material {
    certificate: Vec<u8>,
    pub(super) pkcs8: Zeroizing<Vec<u8>>,
    signing_key: Arc<dyn SigningKey>,
}
impl SignatureScheme {
    pub(super) fn rustls(self) -> rustls::SignatureScheme {
        match self {
            Self::EcdsaP256Sha256 => rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            Self::EcdsaP384Sha384 => rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            Self::Ed25519 => rustls::SignatureScheme::ED25519,
            Self::RsaPssSha256 => rustls::SignatureScheme::RSA_PSS_SHA256,
            Self::RsaPssSha384 => rustls::SignatureScheme::RSA_PSS_SHA384,
            Self::RsaPssSha512 => rustls::SignatureScheme::RSA_PSS_SHA512,
        }
    }
}
impl Material {
    pub(crate) fn generate() -> Result<Self> {
        // Keep the existing key algorithm and certificate generation policy.
        let generated = rcgen::generate_simple_self_signed(vec!["nooboard.local".into()])
            .map_err(|_| Error::InvalidIdentity)?;
        Self::new(
            generated.cert.der().to_vec(),
            generated.signing_key.serialize_der(),
        )
    }
    fn new(certificate: Vec<u8>, pkcs8: Vec<u8>) -> Result<Self> {
        let pkcs8 = Zeroizing::new(pkcs8);
        let provider = rustls::crypto::ring::default_provider();
        let certified = CertifiedKey::from_der(
            vec![CertificateDer::from(certificate.clone())],
            PrivatePkcs8KeyDer::from(pkcs8.to_vec()).into(),
            &provider,
        )
        .map_err(|_| Error::InvalidIdentity)?;
        certified.keys_match().map_err(|_| Error::InvalidIdentity)?;
        Ok(Self {
            certificate,
            pkcs8,
            signing_key: certified.key,
        })
    }
    pub(crate) fn public(&self) -> Result<PublicIdentity> {
        let der = CertificateDer::from(self.certificate.as_slice());
        let parsed = rustls::server::ParsedCertificate::try_from(&der)
            .map_err(|_| Error::InvalidIdentity)?;
        let signature_schemes = [
            SignatureScheme::EcdsaP256Sha256,
            SignatureScheme::EcdsaP384Sha384,
            SignatureScheme::Ed25519,
            SignatureScheme::RsaPssSha256,
            SignatureScheme::RsaPssSha384,
            SignatureScheme::RsaPssSha512,
        ]
        .into_iter()
        .filter(|scheme| self.signing_key.choose_scheme(&[scheme.rustls()]).is_some())
        .collect::<Vec<_>>();
        if signature_schemes.is_empty() {
            return Err(Error::UnsupportedSignature);
        }
        Ok(PublicIdentity {
            id: hex::encode(Sha256::digest(parsed.subject_public_key_info())),
            fingerprint: hex::encode(Sha256::digest(&self.certificate)),
            certificate: self.certificate.clone(),
            signature_schemes,
        })
    }
    pub(crate) fn sign(&self, scheme: SignatureScheme, message: &[u8]) -> Result<Vec<u8>> {
        self.signing_key
            .choose_scheme(&[scheme.rustls()])
            .ok_or(Error::UnsupportedSignature)?
            .sign(message)
            .map_err(|_| Error::Signing)
    }
    /// Unchanged v1 credential bytes: big-endian certificate length, DER cert, PKCS#8 key.
    #[cfg(any(feature = "native-credentials", test))]
    pub(crate) fn export_secret(&self) -> Zeroizing<Vec<u8>> {
        let mut secret = Zeroizing::new((self.certificate.len() as u32).to_be_bytes().to_vec());
        secret.extend_from_slice(&self.certificate);
        secret.extend_from_slice(&self.pkcs8);
        secret
    }
    #[cfg(any(feature = "native-credentials", test))]
    pub(crate) fn from_secret(secret: &[u8]) -> Result<Self> {
        if secret.len() < 5 || secret.len() > 8192 {
            return Err(Error::InvalidIdentity);
        }
        let size = u32::from_be_bytes(secret[..4].try_into().map_err(|_| Error::InvalidIdentity)?)
            as usize;
        if size == 0 || size >= secret.len() - 4 {
            return Err(Error::InvalidIdentity);
        }
        Self::new(secret[4..4 + size].to_vec(), secret[4 + size..].to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SecretBytes, SecretContext};

    #[test]
    fn legacy_credential_bytes_preserve_identity_signature_and_encryption_across_reload() {
        // Construct the pre-migration format without using Material::export_secret.
        let generated = rcgen::generate_simple_self_signed(vec!["nooboard.local".into()]).unwrap();
        let certificate = generated.cert.der();
        let mut legacy = Zeroizing::new((certificate.len() as u32).to_be_bytes().to_vec());
        legacy.extend_from_slice(certificate);
        legacy.extend_from_slice(&generated.signing_key.serialize_der());
        let material = Material::from_secret(&legacy).unwrap();
        assert_eq!(&*material.export_secret(), &*legacy);
        let public = material.public().unwrap();
        assert_eq!(public.certificate, certificate.as_ref());
        assert_eq!(public.fingerprint, hex::encode(Sha256::digest(certificate)));
        let parsed = rustls::server::ParsedCertificate::try_from(certificate).unwrap();
        assert_eq!(
            public.id,
            hex::encode(Sha256::digest(parsed.subject_public_key_info()))
        );
        let signature = material
            .sign(SignatureScheme::EcdsaP256Sha256, b"legacy TLS signature")
            .unwrap();
        ring::signature::UnparsedPublicKey::new(
            &ring::signature::ECDSA_P256_SHA256_ASN1,
            generated.signing_key.public_key_raw(),
        )
        .verify(b"legacy TLS signature", &signature)
        .unwrap();
        let binding = SecretContext {
            purpose: "terminal.ssh-auth".into(),
            associated_data: b"config/field/v1".to_vec(),
        };
        let sealed = material
            .seal(
                binding.clone(),
                SecretBytes::new(b"retained secret".to_vec()),
            )
            .unwrap();
        drop(material);
        let restored = Material::from_secret(&legacy).unwrap();
        assert_eq!(restored.public().unwrap(), public);
        assert_eq!(
            restored
                .open(binding.clone(), sealed.clone())
                .unwrap()
                .expose_secret(),
            b"retained secret"
        );
        // Reissuing only the public certificate does not change the encryption root or device ID.
        let reissued = rcgen::CertificateParams::new(vec!["different-name.local".into()])
            .unwrap()
            .self_signed(&generated.signing_key)
            .unwrap();
        let material = Material::new(
            reissued.der().to_vec(),
            generated.signing_key.serialize_der(),
        )
        .unwrap();
        assert_eq!(material.public().unwrap().id, public.id);
        assert_eq!(
            material.open(binding, sealed).unwrap().expose_secret(),
            b"retained secret"
        );
    }

    #[test]
    fn mismatched_or_truncated_identity_material_is_rejected() {
        let a = Material::generate().unwrap();
        let b = Material::generate().unwrap();
        assert!(matches!(
            Material::new(a.certificate.clone(), b.pkcs8.to_vec()),
            Err(Error::InvalidIdentity)
        ));
        let saved = a.export_secret();
        for length in [0, 3, 4, saved.len() - 1] {
            assert!(matches!(
                Material::from_secret(&saved[..length]),
                Err(Error::InvalidIdentity)
            ));
        }
        assert_eq!(
            a.sign(SignatureScheme::Ed25519, b"wrong scheme")
                .unwrap_err(),
            Error::UnsupportedSignature
        );
    }
}
