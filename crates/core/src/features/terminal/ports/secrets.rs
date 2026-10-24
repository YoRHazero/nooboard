use crate::{
    contracts::{PortFuture, terminal::SshTargetConfigId},
    features::terminal::domain::{config::EncryptedSecret, inputs::SecretInput},
};
#[derive(Clone, Copy)]
pub(crate) enum SecretField {
    Password,
    PrivateKey,
}
pub(crate) trait SecretsPort: Send + Sync {
    fn seal(
        &self,
        id: SshTargetConfigId,
        field: SecretField,
        secret: SecretInput,
    ) -> PortFuture<'_, EncryptedSecret>;
    fn open(
        &self,
        id: SshTargetConfigId,
        field: SecretField,
        secret: EncryptedSecret,
    ) -> PortFuture<'_, SecretInput>;
}
