use crate::{contracts::PortFuture, features::terminal::domain::inputs::SecretInput};
pub(crate) trait PrivateKeyPort: Send + Sync {
    fn unlock(
        &self,
        key: SecretInput,
        passphrase: Option<SecretInput>,
    ) -> PortFuture<'_, SecretInput>;
}
