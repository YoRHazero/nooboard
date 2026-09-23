use crate::{Error, Result};
use nooboard_clipboard::ClipboardService;
use nooboard_network::NetworkService;
use nooboard_storage::StorageService;
#[derive(Default)]
pub(crate) struct Services {
    pub storage: Option<StorageService>,
    pub clipboard: Option<ClipboardService>,
    pub network: Option<NetworkService>,
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
        clipboard.and(storage)
    }
    pub async fn all(&mut self) -> Result<()> {
        let net = self.network().await;
        let rest = self.remaining().await;
        net.and(rest)
    }
}
