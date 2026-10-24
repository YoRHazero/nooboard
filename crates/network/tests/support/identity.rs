//! Explicit caller-owned test identity. Production Network cannot generate or load private keys.
use nooboard_network::{
    IdentitySignatureScheme, IdentitySigner, IdentitySigningError, LocalIdentity,
};
use std::sync::Arc;

pub fn generate() -> LocalIdentity {
    let generated = rcgen::generate_simple_self_signed(vec!["nooboard.local".into()]).unwrap();
    let key =
        rustls::pki_types::PrivatePkcs8KeyDer::from(generated.signing_key.serialize_der()).into();
    let signer = rustls::crypto::ring::sign::any_supported_type(&key).unwrap();
    struct TestSigner(Arc<dyn rustls::sign::SigningKey>);
    impl IdentitySigner for TestSigner {
        fn supported_schemes(&self) -> Vec<IdentitySignatureScheme> {
            vec![IdentitySignatureScheme::EcdsaP256Sha256]
        }
        fn sign(
            &self,
            scheme: IdentitySignatureScheme,
            message: &[u8],
        ) -> Result<Vec<u8>, IdentitySigningError> {
            if scheme != IdentitySignatureScheme::EcdsaP256Sha256 {
                return Err(IdentitySigningError);
            }
            self.0
                .choose_scheme(&[rustls::SignatureScheme::ECDSA_NISTP256_SHA256])
                .ok_or(IdentitySigningError)?
                .sign(message)
                .map_err(|_| IdentitySigningError)
        }
    }
    LocalIdentity::new(generated.cert.der().to_vec(), Arc::new(TestSigner(signer))).unwrap()
}
