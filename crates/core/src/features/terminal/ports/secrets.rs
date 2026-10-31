use crate::{
    contracts::{
        PortFuture,
        terminal::{CommandConfigId, SshTargetConfigId},
    },
    features::terminal::domain::{config::EncryptedSecret, inputs::SecretInput},
};
#[derive(Clone, Copy)]
pub(crate) enum SecretField {
    Password,
    PrivateKey,
}
pub(crate) enum SecretScope {
    Ssh(SshTargetConfigId, SecretField),
    CommandInput(CommandConfigId, String),
}
pub(crate) trait SecretsPort: Send + Sync {
    fn seal(&self, scope: SecretScope, secret: SecretInput) -> PortFuture<'_, EncryptedSecret>;
    fn open(&self, scope: SecretScope, secret: EncryptedSecret) -> PortFuture<'_, SecretInput>;
}
