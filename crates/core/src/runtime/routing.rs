//! Own the unique network event receiver; never wait for storage or clipboard work here.
use crate::{Error, Result};
use nooboard_network::{ApplicationOutcome, Network, NetworkEvent, NetworkEvents, ReceiveDecision};
use tokio::sync::{mpsc, watch};
pub(crate) async fn run(
    network: Network,
    mut inbox: NetworkEvents,
    devices: mpsc::Sender<NetworkEvent>,
    sync: mpsc::Sender<NetworkEvent>,
    mut stop: watch::Receiver<bool>,
) -> Result<()> {
    loop {
        if *stop.borrow() {
            return Ok(());
        }
        let event = tokio::select! {biased;_=stop.changed()=>return Ok(()),event=inbox.next_event()=>event.ok_or(Error::Stopped)?};
        let destination = match &event {
            NetworkEvent::PairingOffered { .. } | NetworkEvent::PairingVerified { .. } => &devices,
            _ => &sync,
        };
        if let Err(error) = destination.try_send(event) {
            reject(&network, error.into_inner()).await;
        }
    }
}
pub(crate) async fn reject(network: &Network, event: NetworkEvent) {
    match event {
        NetworkEvent::PairingOffered { id, .. } => {
            let _ = network.cancel_pairing(id).await;
        }
        NetworkEvent::PairingVerified { id, .. } => {
            let _ = network.complete_pairing(id, false).await;
        }
        NetworkEvent::IncomingOffer { id, .. } => {
            let _ = network.decide_incoming(id, ReceiveDecision::Reject).await;
        }
        NetworkEvent::ContentReady { id, .. } => {
            let _ = network
                .complete_incoming(id, ApplicationOutcome::Rejected)
                .await;
        }
    }
}
