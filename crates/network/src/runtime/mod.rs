//! Assemble feature runtimes and supervise their lifetime. No feature state machine lives here.
mod model;
use crate::{
    connections, discovery,
    error::{Failure, InternalResult as Result},
    event::NetworkEvent,
    identity::{self, PublicIdentity},
    options::Options,
    pairing, transfer,
};
pub use model::{NetworkStatus, ServiceState};
use tokio::{
    sync::{mpsc, watch},
    task::{JoinHandle, JoinSet},
};

pub(crate) struct Started {
    pub connections: connections::runtime::Handle,
    pub discovery: discovery::runtime::Handle,
    pub pairing: pairing::runtime::Handle,
    pub transfer: transfer::runtime::Handle,
    pub inbox: mpsc::Receiver<NetworkEvent>,
    pub status: watch::Receiver<NetworkStatus>,
    pub stop: watch::Sender<bool>,
    pub task: JoinHandle<Result<()>>,
}
pub(crate) async fn start(options: Options) -> Result<Started> {
    options.validate()?;
    let identity = identity::load(options.identity.clone()).await?;
    let public = PublicIdentity::from_identity(&identity, options.device_name.clone())?;
    let (stop, stopped) = watch::channel(false);
    let (events, inbox) = mpsc::channel(options.event_capacity);
    let (connections_runtime, connections, links) =
        connections::runtime::Runtime::bind(&options, identity, stopped.clone()).await?;
    let (pairing_runtime, pairing) = pairing::runtime::Runtime::bind(
        options.pairing_listen,
        pairing::Contact {
            device_name: public.device_name.clone(),
            certificate: public.certificate.clone(),
            sync_port: connections.address.port(),
        },
        options.queue_capacity,
        events.clone(),
        stopped.clone(),
        connections.requests.clone(),
    )
    .await?;
    let (discovery_runtime, discovery) = discovery::runtime::Runtime::new(
        public.id.clone(),
        public.device_name.clone(),
        pairing.address,
        connections.address,
        options.discovery,
        options.queue_capacity,
        stopped.clone(),
    )?;
    let (transfer_runtime, transfer) =
        transfer::runtime::Runtime::new(&options, links, events, stopped.clone())?;
    let initial = NetworkStatus {
        state: ServiceState::Running,
        identity: public,
        listen_address: connections.address,
        pairing_address: pairing.address,
        nearby: vec![],
        connections: connections.status.borrow().clone(),
        pairings: vec![],
        transfers: vec![],
    };
    let (status, receiver) = watch::channel(initial);
    let mut peers = connections.status.clone();
    let mut nearby = discovery.status.clone();
    let mut pairings = pairing.status.clone();
    let mut transfers = transfer.status.clone();
    let hints = connections.hints.clone();
    let signal = stop.clone();
    let mut stopped = stopped;
    // Only the owner controls lifetime. Keep request queues open even if the
    // application temporarily holds no request handles.
    let request_lifetime = (
        connections.requests.clone(),
        discovery.requests.clone(),
        pairing.requests.clone(),
        transfer.requests.clone(),
    );
    let task = tokio::spawn(async move {
        let _request_lifetime = request_lifetime;
        // Cancelling the supervisor also signals child runtimes before their task guards drop.
        struct Stop(watch::Sender<bool>);
        impl Drop for Stop {
            fn drop(&mut self) {
                self.0.send_replace(true);
            }
        }
        let _guard = Stop(signal.clone());
        let mut tasks = JoinSet::new();
        tasks.spawn(connections_runtime.run());
        tasks.spawn(pairing_runtime.run());
        tasks.spawn(discovery_runtime.run());
        tasks.spawn(transfer_runtime.run());
        let mut failure = None;
        loop {
            if *stopped.borrow() {
                break;
            }
            tokio::select! {
                _ = stopped.changed() => break,
                result = tasks.join_next() => {
                    if !*stopped.borrow() {
                        failure = Some(match result { Some(Ok(Err(error))) => error, _ => Failure::Internal });
                    }
                    break;
                },
                _ = peers.changed() => {},
                _ = nearby.changed() => {},
                _ = pairings.changed() => {},
                _ = transfers.changed() => {},
            }
            let discovered = discovery_snapshot(&mut nearby, &hints);
            status.send_modify(|s| {
                s.connections = peers.borrow_and_update().clone();
                s.nearby = discovered;
                s.pairings = pairings.borrow_and_update().clone();
                let snapshot = transfers.borrow_and_update();
                s.transfers = snapshot.rows.clone();
                for peer in &mut s.connections {
                    peer.accepting =
                        peer.connected && snapshot.ready.get(&peer.peer) == Some(&peer.generation);
                }
            });
        }
        signal.send_replace(true);
        // Feature runtimes drain owned workers, including non-cancellable file publication.
        while let Some(result) = tasks.join_next().await {
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    if failure.is_none() {
                        failure = Some(error);
                    }
                }
                Err(_) => {
                    if failure.is_none() {
                        failure = Some(Failure::Internal);
                    }
                }
            }
        }
        status.send_modify(|s| {
            s.connections = peers.borrow().clone();
            s.nearby = nearby.borrow().clone();
            s.pairings = pairings.borrow().clone();
            s.transfers = transfers.borrow().rows.clone();
            s.state = failure
                .as_ref()
                .map_or(ServiceState::Stopped, |e| ServiceState::Failed(e.kind()));
        });
        match failure {
            Some(error) => Err(error),
            None => Ok(()),
        }
    });
    Ok(Started {
        connections,
        discovery,
        pairing,
        transfer,
        inbox,
        status: receiver,
        stop,
        task,
    })
}

// `changed().await` marks a watch update as seen. Forward its value independently
// of which select branch woke the supervisor, including simultaneous status changes.
fn discovery_snapshot(
    nearby: &mut watch::Receiver<Vec<discovery::NearbyDevice>>,
    hints: &watch::Sender<Vec<discovery::NearbyDevice>>,
) -> Vec<discovery::NearbyDevice> {
    let snapshot = nearby.borrow_and_update().clone();
    hints.send_if_modified(|known| {
        if *known == snapshot {
            return false;
        }
        known.clone_from(&snapshot);
        true
    });
    snapshot
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn discovery_hints_reach_connections_before_and_after_watch_notification_is_consumed() {
        let (source, mut nearby) = watch::channel(vec![]);
        let (hints, receiver) = watch::channel(vec![]);
        let device = discovery::NearbyDevice {
            key: "device.local.".into(),
            noob_id: "peer".into(),
            device_name: "peer".into(),
            addresses: vec!["127.0.0.1:24817".into()],
            sync_port: 24816,
        };
        source.send_replace(vec![device.clone()]);
        nearby.changed().await.unwrap();
        assert!(!nearby.has_changed().unwrap());
        assert_eq!(
            discovery_snapshot(&mut nearby, &hints),
            vec![device.clone()]
        );
        assert_eq!(*receiver.borrow(), vec![device]);
        source.send_replace(vec![]);
        assert!(discovery_snapshot(&mut nearby, &hints).is_empty());
        assert!(receiver.borrow().is_empty());
    }
}
