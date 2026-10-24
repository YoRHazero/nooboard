use crate::{
    Error,
    contracts::{PortFuture, terminal::SshTargetConfigId},
    features::terminal::{
        domain::{config::EncryptedSecret, inputs::SecretInput},
        ports::secrets::{SecretField, SecretsPort},
    },
};
pub(crate) struct SecretsAdapter(pub nooboard_identity::Identity);
fn context(id: SshTargetConfigId, field: SecretField) -> nooboard_identity::SecretContext {
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
impl SecretsPort for SecretsAdapter {
    fn seal(
        &self,
        id: SshTargetConfigId,
        field: SecretField,
        secret: SecretInput,
    ) -> PortFuture<'_, EncryptedSecret> {
        Box::pin(async move {
            let sealed = self
                .0
                .seal(
                    context(id, field),
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
    fn open(
        &self,
        id: SshTargetConfigId,
        field: SecretField,
        secret: EncryptedSecret,
    ) -> PortFuture<'_, SecretInput> {
        Box::pin(async move {
            let nonce = secret.nonce.try_into().map_err(|_| Error::Configuration)?;
            let plaintext = self
                .0
                .open(
                    context(id, field),
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
