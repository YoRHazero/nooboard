use crate::{
    Error, PairingError, PairingFailure, Result,
    configuration::runtime::{Change, Handle},
};
use nooboard_network::{ErrorKind, Network, PairingId, TrustedPeer};
pub(crate) async fn verified(
    network: &Network,
    config: &Handle,
    id: PairingId,
    peer: TrustedPeer,
    expected: Option<&str>,
) -> Result<()> {
    if expected.is_some_and(|expected| expected != peer.identity.id) {
        network.complete_pairing(id, false).await?;
        return Err(Error::AlreadyPaired);
    }
    if let Err(error) = config.change(Change::SavePeer(peer)).await {
        let _ = network.complete_pairing(id, false).await;
        return Err(error);
    }
    network.complete_pairing(id, true).await?;
    Ok(())
}
pub(crate) fn failure(error: ErrorKind) -> PairingFailure {
    PairingFailure::Network(match error {
        ErrorKind::Timeout => PairingError::Timeout,
        ErrorKind::Cancelled => PairingError::Cancelled,
        ErrorKind::Busy => PairingError::Busy,
        ErrorKind::Unavailable | ErrorKind::Stopped => PairingError::Disconnected,
        _ => PairingError::Protocol,
    })
}
pub(crate) async fn addresses(address: &str) -> Result<Vec<std::net::SocketAddr>> {
    if address.len() > 512 {
        return Err(Error::Configuration);
    }
    let values = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::net::lookup_host(address),
    )
    .await
    .map_err(|_| Error::Offline)?
    .map_err(|_| Error::Configuration)?;
    let mut out = Vec::new();
    for value in values {
        if value.port() != 0
            && !value.ip().is_unspecified()
            && !value.ip().is_multicast()
            && !out.contains(&value)
        {
            out.push(value);
        }
        if out.len() == 16 {
            break;
        }
    }
    if out.is_empty() {
        Err(Error::Configuration)
    } else {
        Ok(out)
    }
}
pub(crate) async fn trusted(peer: &crate::configuration::model::Peer) -> Result<TrustedPeer> {
    let mut trusted = peer.trusted.clone();
    if let Some(address) = &peer.settings.address {
        trusted.addresses = addresses(address).await?;
    }
    Ok(trusted)
}
