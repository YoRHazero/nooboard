use crate::contracts::{PortFuture, network::*};
use crate::features::devices::ports::network::NetworkPort;
use std::sync::Arc;
use tokio::sync::watch;
pub(crate) struct NetworkAdapter(nooboard_network::Network);
pub(crate) fn port(network: nooboard_network::Network) -> Arc<dyn NetworkPort> {
    Arc::new(NetworkAdapter(network))
}
impl NetworkPort for NetworkAdapter {
    fn status(&self) -> NetworkStatus {
        self.0.status()
    }
    fn subscribe(&self) -> watch::Receiver<NetworkStatus> {
        self.0.subscribe()
    }
    fn refresh_discovery(&self) -> PortFuture<'_, ()> {
        Box::pin(async move { self.0.refresh_discovery().await.map_err(Into::into) })
    }
    fn enable_discovery(&self, enabled: bool) -> PortFuture<'_, ()> {
        Box::pin(async move { self.0.enable_discovery(enabled).await.map_err(Into::into) })
    }
    fn local_addresses(&self) -> PortFuture<'_, Vec<LocalAddress>> {
        Box::pin(async move { self.0.local_addresses().await.map_err(Into::into) })
    }
    fn trust_peer(&self, peer: TrustedPeer) -> PortFuture<'_, ()> {
        Box::pin(async move { self.0.trust_peer(peer).await.map_err(Into::into) })
    }
    fn revoke_peer(&self, peer: String) -> PortFuture<'_, ()> {
        Box::pin(async move { self.0.revoke_peer(peer).await.map_err(Into::into) })
    }
    fn start_pairing(&self, addresses: Vec<std::net::SocketAddr>) -> PortFuture<'_, PairingId> {
        Box::pin(async move { self.0.start_pairing(addresses).await.map_err(Into::into) })
    }
    fn accept_pairing(&self, id: PairingId) -> PortFuture<'_, ()> {
        Box::pin(async move { self.0.accept_pairing(id).await.map_err(Into::into) })
    }
    fn cancel_pairing(&self, id: PairingId) -> PortFuture<'_, ()> {
        Box::pin(async move { self.0.cancel_pairing(id).await.map_err(Into::into) })
    }
    fn submit_pairing_code(&self, id: PairingId, code: String) -> PortFuture<'_, ()> {
        Box::pin(async move {
            self.0
                .submit_pairing_code(id, code)
                .await
                .map_err(Into::into)
        })
    }
    fn complete_pairing(&self, id: PairingId, saved: bool) -> PortFuture<'_, ()> {
        Box::pin(async move { self.0.complete_pairing(id, saved).await.map_err(Into::into) })
    }
    fn resolve<'a>(&'a self, address: &'a str) -> PortFuture<'a, Vec<std::net::SocketAddr>> {
        Box::pin(super::resolver::addresses(address))
    }
}
