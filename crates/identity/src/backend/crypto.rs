use super::Material;
use crate::{Error, Result, SealedSecret, SecretBytes, SecretContext};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::Zeroizing;

impl Material {
    fn cipher(&self, context: &SecretContext) -> Result<XChaCha20Poly1305> {
        let derivation =
            Hkdf::<Sha256>::new(Some(b"nooboard.identity.local-secrets.v1"), &self.pkcs8);
        let mut info = b"nooboard.identity.encryption-key.v1".to_vec();
        info.extend_from_slice(&(context.purpose.len() as u64).to_be_bytes());
        info.extend_from_slice(context.purpose.as_bytes());
        let mut key = Zeroizing::new([0u8; 32]);
        derivation
            .expand(&info, key.as_mut())
            .map_err(|_| Error::Encryption)?;
        XChaCha20Poly1305::new_from_slice(key.as_ref()).map_err(|_| Error::Encryption)
    }
    pub(crate) fn seal(
        &self,
        context: SecretContext,
        plaintext: SecretBytes,
    ) -> Result<SealedSecret> {
        let mut nonce = [0u8; 24];
        rustls::crypto::ring::default_provider()
            .secure_random
            .fill(&mut nonce)
            .map_err(|_| Error::Encryption)?;
        let aad = binding(&context);
        let ciphertext = self
            .cipher(&context)?
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: plaintext.expose_secret(),
                    aad: &aad,
                },
            )
            .map_err(|_| Error::Encryption)?;
        Ok(SealedSecret {
            version: 1,
            nonce,
            ciphertext,
        })
    }
    pub(crate) fn open(&self, context: SecretContext, sealed: SealedSecret) -> Result<SecretBytes> {
        if sealed.version != 1 {
            return Err(Error::UnsupportedVersion);
        }
        let aad = binding(&context);
        self.cipher(&context)?
            .decrypt(
                XNonce::from_slice(&sealed.nonce),
                Payload {
                    msg: &sealed.ciphertext,
                    aad: &aad,
                },
            )
            .map(SecretBytes::new)
            .map_err(|_| Error::Authentication)
    }
}
fn binding(context: &SecretContext) -> Vec<u8> {
    let mut aad = b"nooboard.identity.sealed-secret.v1".to_vec();
    aad.extend_from_slice(&(context.purpose.len() as u64).to_be_bytes());
    aad.extend_from_slice(context.purpose.as_bytes());
    aad.extend_from_slice(&(context.associated_data.len() as u64).to_be_bytes());
    aad.extend_from_slice(&context.associated_data);
    aad
}
