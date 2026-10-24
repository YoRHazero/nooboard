use crate::{
    Error,
    contracts::PortFuture,
    features::terminal::{domain::inputs::SecretInput, ports::keys::PrivateKeyPort},
};
pub(crate) struct PrivateKeyAdapter;
impl PrivateKeyPort for PrivateKeyAdapter {
    fn unlock(
        &self,
        key: SecretInput,
        passphrase: Option<SecretInput>,
    ) -> PortFuture<'_, SecretInput> {
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                let key = std::str::from_utf8(key.expose()).map_err(|_| Error::Configuration)?;
                let passphrase = passphrase
                    .as_ref()
                    .map(|p| std::str::from_utf8(p.expose()))
                    .transpose()
                    .map_err(|_| Error::Configuration)?;
                let key = russh::keys::decode_secret_key(key, passphrase)
                    .map_err(|_| Error::Configuration)?;
                let text = key
                    .to_openssh(russh::keys::ssh_key::LineEnding::LF)
                    .map_err(|_| Error::Configuration)?;
                Ok(SecretInput::new(text.as_bytes().to_vec()))
            })
            .await
            .map_err(|_| Error::Internal)?
        })
    }
}
