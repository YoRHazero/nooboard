use crate::{
    Error,
    contracts::PortFuture,
    features::terminal::{
        domain::{config::EncryptedSecret, inputs::SecretInput},
        ports::secrets::{SecretField, SecretScope, SecretsPort},
    },
};
pub(crate) struct SecretsAdapter(pub nooboard_identity::Identity);
fn context(scope: SecretScope) -> nooboard_identity::SecretContext {
    match scope {
        SecretScope::Ssh(id, field) => {
            // Preserve the existing authentication context byte-for-byte.
            let mut associated_data = id.as_uuid().as_bytes().to_vec();
            associated_data.push(match field {
                SecretField::Password => 1,
                SecretField::PrivateKey => 2,
            });
            associated_data.extend_from_slice(&1u32.to_be_bytes());
            nooboard_identity::SecretContext {
                purpose: "terminal.ssh-auth".into(),
                associated_data,
            }
        }
        SecretScope::CommandInput(id, prefix) => {
            let mut associated_data = id.as_uuid().as_bytes().to_vec();
            associated_data.extend_from_slice(&1u32.to_be_bytes());
            associated_data.extend_from_slice(prefix.as_bytes());
            nooboard_identity::SecretContext {
                purpose: "terminal.command-auto-input".into(),
                associated_data,
            }
        }
    }
}
impl SecretsPort for SecretsAdapter {
    fn seal(&self, scope: SecretScope, secret: SecretInput) -> PortFuture<'_, EncryptedSecret> {
        Box::pin(async move {
            let sealed = self
                .0
                .seal(
                    context(scope),
                    nooboard_identity::SecretBytes::new(secret.expose().to_vec()),
                )
                .await?;
            Ok(EncryptedSecret {
                version: sealed.version,
                nonce: sealed.nonce.to_vec(),
                ciphertext: sealed.ciphertext,
            })
        })
    }
    fn open(&self, scope: SecretScope, secret: EncryptedSecret) -> PortFuture<'_, SecretInput> {
        Box::pin(async move {
            let nonce = secret.nonce.try_into().map_err(|_| Error::Configuration)?;
            let plaintext = self
                .0
                .open(
                    context(scope),
                    nooboard_identity::SealedSecret {
                        version: secret.version,
                        nonce,
                        ciphertext: secret.ciphertext,
                    },
                )
                .await?;
            Ok(SecretInput::new(plaintext.expose_secret().to_vec()))
        })
    }
}
