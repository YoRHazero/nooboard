use crate::{Error, Result};
use nooboard_clipboard::ClipboardService;
use nooboard_identity::IdentityService;
use nooboard_network::NetworkService;
use nooboard_storage::StorageService;
#[derive(Default)]
pub(crate) struct Services {
    pub storage: Option<StorageService>,
    pub clipboard: Option<ClipboardService>,
    pub network: Option<NetworkService>,
    pub identity: Option<IdentityService>,
}
impl Services {
    pub async fn network(&mut self) -> Result<()> {
        if let Some(service) = self.network.take() {
            service.shutdown().await?;
        }
        Ok(())
    }
    pub async fn remaining(&mut self) -> Result<()> {
        let clipboard = match self.clipboard.take() {
            Some(service) => service.shutdown().await.map_err(Error::from),
            None => Ok(()),
        };
        let storage = match self.storage.take() {
            Some(service) => service.shutdown().await.map_err(Error::from),
            None => Ok(()),
        };
        // Consumers have finished before the private identity worker is released.
        let identity = match self.identity.as_mut() {
            Some(service) => service.shutdown().await.map_err(Error::from),
            None => Ok(()),
        };
        if identity.is_ok() {
            self.identity.take();
        }
        clipboard.and(storage).and(identity)
    }
    pub async fn all(&mut self) -> Result<()> {
        let net = self.network().await;
        let rest = self.remaining().await;
        net.and(rest)
    }
}
