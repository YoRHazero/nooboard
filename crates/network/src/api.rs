//! The only public facade. Feature managers, wire protocols and credentials remain private.
use crate::{
    connections, discovery,
    error::{Failure, InternalResult},
    pairing, runtime, transfer,
};
pub use crate::{
    connections::ConnectionStatus,
    discovery::{LocalAddress, NearbyDevice},
    error::{Error, ErrorKind},
    event::NetworkEvent,
    identity::{IdentityOptions, PublicIdentity, TrustedPeer},
    options::Options,
    pairing::{PairingId, PairingStatus, Stage as PairingStage},
    runtime::{NetworkStatus, ServiceState},
    transfer::{
        ApplicationOutcome, ContentKind, FileEntry, IncomingId, MAX_TEXT_BYTES, OutgoingContent,
        QueuePolicy, ReceiveDecision, ReceivedContent, SendRequest, TransferId, TransferStage,
        TransferStatus,
    },
};
use std::net::SocketAddr;
use tokio::{
    sync::{mpsc, oneshot, watch},
    task::JoinHandle,
};
pub type Result<T> = std::result::Result<T, Error>;

/// Owns the service lifetime. Drop signals stop; `shutdown` waits for cleanup.
/// Request handles and the event receiver may remain alive during shutdown.
pub struct NetworkService {
    stop: watch::Sender<bool>,
    task: Option<JoinHandle<InternalResult<()>>>,
}
impl NetworkService {
    /// Requires a Tokio runtime. Returns the owner, a cloneable request handle and
    /// the single event receiver after identity, listeners and discovery are ready.
    pub async fn start(options: Options) -> Result<(Self, Network, NetworkEvents)> {
        let parts = runtime::start(options).await?;
        let stopped = parts.stop.subscribe();
        let network = Network {
            connections: parts.connections.requests,
            discovery: parts.discovery.requests,
            pairing: parts.pairing.requests,
            transfer: parts.transfer.requests,
            status: parts.status,
            stopped: stopped.clone(),
        };
        let events = NetworkEvents {
            inbox: parts.inbox,
            stopped,
        };
        let service = Self {
            stop: parts.stop,
            task: Some(parts.task),
        };
        Ok((service, network, events))
    }
    /// Stop admission and await owned tasks, including file publication already in progress.
    /// Existing request handles do not prevent shutdown.
    pub async fn shutdown(mut self) -> Result<()> {
        self.stop.send_replace(true);
        if let Some(task) = self.task.take() {
            task.await.map_err(|_| Error::from(Failure::Internal))??;
        }
        Ok(())
    }
}
impl Drop for NetworkService {
    fn drop(&mut self) {
        self.stop.send_replace(true);
    }
}

/// Cloneable operation entry point. Clones share the same runtime and queues.
/// Dropping a handle does not stop the service; shutdown belongs to `NetworkService`.
#[derive(Clone)]
pub struct Network {
    connections: mpsc::Sender<connections::runtime::Request>,
    discovery: mpsc::Sender<discovery::runtime::Request>,
    pairing: mpsc::Sender<pairing::runtime::Request>,
    transfer: mpsc::Sender<transfer::runtime::Request>,
    status: watch::Receiver<NetworkStatus>,
    stopped: watch::Receiver<bool>,
}
impl Network {
    pub fn status(&self) -> NetworkStatus {
        self.status.borrow().clone()
    }
    /// Latest state; intermediate progress may coalesce. Never carries content bodies.
    pub fn subscribe(&self) -> watch::Receiver<NetworkStatus> {
        self.status.clone()
    }
    pub async fn refresh_discovery(&self) -> Result<()> {
        self.request(&self.discovery, |reply| {
            discovery::runtime::Request::Refresh { reply }
        })
        .await
    }
    pub async fn enable_discovery(&self, enabled: bool) -> Result<()> {
        self.request(&self.discovery, |reply| {
            discovery::runtime::Request::Enable { enabled, reply }
        })
        .await
    }
    pub async fn local_addresses(&self) -> Result<Vec<LocalAddress>> {
        self.request(&self.discovery, |reply| {
            discovery::runtime::Request::Addresses { reply }
        })
        .await
    }
    /// Install a caller-authorized, already-persisted public trust record.
    pub async fn trust_peer(&self, peer: TrustedPeer) -> Result<()> {
        self.request(&self.connections, |reply| {
            connections::runtime::Request::Trust { peer, reply }
        })
        .await
    }
    /// Immediately revoke runtime trust and close sessions. The caller also removes its saved record.
    pub async fn revoke_peer(&self, peer: impl Into<String>) -> Result<()> {
        let peer = peer.into();
        self.request(&self.connections, |reply| {
            connections::runtime::Request::Revoke { peer, reply }
        })
        .await
    }
    pub async fn connect(&self, peer: impl Into<String>) -> Result<()> {
        let peer = peer.into();
        self.request(&self.connections, |reply| {
            connections::runtime::Request::Enable {
                peer,
                enabled: true,
                reply,
            }
        })
        .await
    }
    /// Suspend dialing and reject new sessions until `connect` re-enables this trusted peer.
    pub async fn disconnect(&self, peer: impl Into<String>) -> Result<()> {
        let peer = peer.into();
        self.request(&self.connections, |reply| {
            connections::runtime::Request::Enable {
                peer,
                enabled: false,
                reply,
            }
        })
        .await
    }
    pub async fn start_pairing(&self, addresses: Vec<SocketAddr>) -> Result<PairingId> {
        self.request(&self.pairing, |reply| pairing::runtime::Request::Start {
            addresses,
            reply,
        })
        .await
    }
    pub async fn accept_pairing(&self, id: PairingId) -> Result<()> {
        self.request(&self.pairing, |reply| pairing::runtime::Request::Accept {
            id,
            reply,
        })
        .await
    }
    pub async fn submit_pairing_code(&self, id: PairingId, code: String) -> Result<()> {
        let code = zeroize::Zeroizing::new(code);
        self.request(&self.pairing, |reply| pairing::runtime::Request::Code {
            id,
            code,
            reply,
        })
        .await
    }
    pub async fn cancel_pairing(&self, id: PairingId) -> Result<()> {
        self.request(&self.pairing, |reply| pairing::runtime::Request::Cancel {
            id,
            reply,
        })
        .await
    }
    /// Report whether the caller durably saved the exact `PairingVerified` record.
    pub async fn complete_pairing(&self, id: PairingId, saved: bool) -> Result<()> {
        self.request(&self.pairing, |reply| pairing::runtime::Request::Saved {
            id,
            saved,
            reply,
        })
        .await
    }
    /// Returns an accepted operation ID, not proof of remote delivery/application.
    pub async fn send(&self, input: SendRequest) -> Result<TransferId> {
        input.validate()?;
        self.request(&self.transfer, |reply| transfer::runtime::Request::Send {
            input,
            reply,
        })
        .await
    }
    /// Request cancellation for all destinations; consult status for its actual outcome.
    pub async fn cancel_transfer(&self, id: TransferId) -> Result<()> {
        self.request(&self.transfer, |reply| transfer::runtime::Request::Cancel {
            id,
            reply,
        })
        .await
    }
    /// Cancel one outgoing destination without cancelling the remaining destinations.
    pub async fn cancel_delivery(&self, peer: String, id: TransferId) -> Result<()> {
        self.request(&self.transfer, |reply| {
            transfer::runtime::Request::CancelDelivery { peer, id, reply }
        })
        .await
    }
    /// Cancel incoming work identified by its sender and operation. Application
    /// already dispatched and published files cannot be rolled back.
    pub async fn cancel_incoming(&self, peer: String, id: TransferId) -> Result<()> {
        self.request(&self.transfer, |reply| {
            transfer::runtime::Request::CancelIncoming { peer, id, reply }
        })
        .await
    }
    pub async fn decide_incoming(&self, id: IncomingId, decision: ReceiveDecision) -> Result<()> {
        self.request(&self.transfer, |reply| transfer::runtime::Request::Decide {
            id,
            decision,
            reply,
        })
        .await
    }
    pub async fn complete_incoming(
        &self,
        id: IncomingId,
        outcome: ApplicationOutcome,
    ) -> Result<()> {
        self.request(&self.transfer, |reply| {
            transfer::runtime::Request::Complete { id, outcome, reply }
        })
        .await
    }
    /// Controls admission of new incoming content; already admitted work retains its outcome semantics.
    pub async fn set_accepting(&self, accepting: bool) -> Result<()> {
        self.request(&self.transfer, |reply| {
            transfer::runtime::Request::Accepting { accepting, reply }
        })
        .await
    }
    async fn request<T, Q>(
        &self,
        sender: &mpsc::Sender<Q>,
        make: impl FnOnce(oneshot::Sender<InternalResult<T>>) -> Q,
    ) -> Result<T> {
        let mut stopped = self.stopped.clone();
        if *stopped.borrow() {
            return Err(Failure::Closed.into());
        }
        let (reply, receive) = oneshot::channel();
        tokio::select! {
            biased;
            _ = stopped.changed() => return Err(Failure::Closed.into()),
            result = sender.send(make(reply)) => result.map_err(|_| Failure::Closed)?,
        }
        receive
            .await
            .map_err(|_| Failure::Closed)?
            .map_err(Into::into)
    }
}
/// Single consumer for work that requires an application decision or result.
/// Move this receiver into the consumer task; it is not cloneable.
pub struct NetworkEvents {
    inbox: mpsc::Receiver<NetworkEvent>,
    stopped: watch::Receiver<bool>,
}
impl NetworkEvents {
    /// Cancellation-safe. Shutdown wakes a pending call and returns `None`,
    /// including when the queue still contains work that has not been dispatched.
    pub async fn next_event(&mut self) -> Option<NetworkEvent> {
        if *self.stopped.borrow() {
            return None;
        }
        tokio::select! {
            biased;
            _ = self.stopped.changed() => None,
            event = self.inbox.recv() => event,
        }
    }
}
