use crate::contracts::{PortFuture, network::*};
use tokio::sync::watch;
pub(crate) trait NetworkPort: Send + Sync {
    fn status(&self) -> NetworkStatus;
    fn subscribe(&self) -> watch::Receiver<NetworkStatus>;
    fn send(&self, input: SendRequest) -> PortFuture<'_, TransferId>;
    fn set_accepting(&self, enabled: bool) -> PortFuture<'_, ()>;
    fn set_paused(&self, paused: bool) -> PortFuture<'_, ()>;
    fn cancel_transfer(&self, id: TransferId) -> PortFuture<'_, ()>;
    fn cancel_delivery(&self, peer: String, id: TransferId) -> PortFuture<'_, ()>;
    fn cancel_incoming(&self, peer: String, id: TransferId) -> PortFuture<'_, ()>;
    fn complete_incoming(&self, id: IncomingId, outcome: ApplicationOutcome) -> PortFuture<'_, ()>;
    fn decide_incoming(&self, id: IncomingId, decision: ReceiveDecision) -> PortFuture<'_, ()>;
}
