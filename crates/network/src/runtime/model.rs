use crate::{
    connections::ConnectionStatus, discovery::NearbyDevice, error::ErrorKind,
    identity::PublicIdentity, pairing::PairingStatus, transfer::TransferStatus,
};
use std::net::SocketAddr;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceState {
    Running,
    Stopped,
    Failed(ErrorKind),
}
#[derive(Clone, Debug)]
pub struct NetworkStatus {
    pub state: ServiceState,
    pub identity: PublicIdentity,
    pub listen_address: SocketAddr,
    pub pairing_address: SocketAddr,
    pub nearby: Vec<NearbyDevice>,
    pub connections: Vec<ConnectionStatus>,
    pub pairings: Vec<PairingStatus>,
    pub transfers: Vec<TransferStatus>,
}
