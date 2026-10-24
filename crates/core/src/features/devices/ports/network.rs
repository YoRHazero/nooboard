use crate::contracts::{PortFuture, network::*};
use tokio::sync::watch;
pub(crate) trait NetworkPort: Send + Sync {
    fn status(&self) -> NetworkStatus;
    fn subscribe(&self) -> watch::Receiver<NetworkStatus>;
    fn refresh_discovery(&self) -> PortFuture<'_, ()>;
    fn enable_discovery(&self, enabled: bool) -> PortFuture<'_, ()>;
    fn local_addresses(&self) -> PortFuture<'_, Vec<LocalAddress>>;
    fn trust_peer(&self, peer: TrustedPeer) -> PortFuture<'_, ()>;
    fn revoke_peer(&self, peer: String) -> PortFuture<'_, ()>;
    fn start_pairing(&self, addresses: Vec<std::net::SocketAddr>) -> PortFuture<'_, PairingId>;
    fn accept_pairing(&self, id: PairingId) -> PortFuture<'_, ()>;
    fn cancel_pairing(&self, id: PairingId) -> PortFuture<'_, ()>;
    fn submit_pairing_code(&self, id: PairingId, code: String) -> PortFuture<'_, ()>;
    fn complete_pairing(&self, id: PairingId, saved: bool) -> PortFuture<'_, ()>;
    fn resolve<'a>(&'a self, address: &'a str) -> PortFuture<'a, Vec<std::net::SocketAddr>>;
}
