use crate::contracts::{PortFuture, network::*};
use crate::features::clipboard::ports::network::NetworkPort;
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
    fn send(&self, input: SendRequest) -> PortFuture<'_, TransferId> {
        Box::pin(async move { self.0.send(input).await.map_err(Into::into) })
    }
    fn set_accepting(&self, enabled: bool) -> PortFuture<'_, ()> {
        Box::pin(async move { self.0.set_accepting(enabled).await.map_err(Into::into) })
    }
    fn set_paused(&self, paused: bool) -> PortFuture<'_, ()> {
        Box::pin(async move { self.0.set_paused(paused).await.map_err(Into::into) })
    }
    fn cancel_transfer(&self, id: TransferId) -> PortFuture<'_, ()> {
        Box::pin(async move { self.0.cancel_transfer(id).await.map_err(Into::into) })
    }
    fn cancel_delivery(&self, peer: String, id: TransferId) -> PortFuture<'_, ()> {
        Box::pin(async move { self.0.cancel_delivery(peer, id).await.map_err(Into::into) })
    }
    fn cancel_incoming(&self, peer: String, id: TransferId) -> PortFuture<'_, ()> {
        Box::pin(async move { self.0.cancel_incoming(peer, id).await.map_err(Into::into) })
    }
    fn complete_incoming(&self, id: IncomingId, outcome: ApplicationOutcome) -> PortFuture<'_, ()> {
        Box::pin(async move {
            self.0
                .complete_incoming(id, outcome)
                .await
                .map_err(Into::into)
        })
    }
    fn decide_incoming(&self, id: IncomingId, decision: ReceiveDecision) -> PortFuture<'_, ()> {
        Box::pin(async move {
            self.0
                .decide_incoming(id, decision)
                .await
                .map_err(Into::into)
        })
    }
}
