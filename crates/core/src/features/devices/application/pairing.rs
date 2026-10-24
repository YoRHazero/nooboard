use crate::contracts::network::{ErrorKind, PairingId, TrustedPeer};
use crate::{
    Error, PairingError, PairingFailure, Result,
    configuration::{domain::change::Change, ports::settings::ConfigurationPort},
};
pub(crate) async fn verified(
    network: &dyn crate::features::devices::ports::network::NetworkPort,
    config: &dyn ConfigurationPort,
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
pub(crate) async fn trusted(
    network: &dyn crate::features::devices::ports::network::NetworkPort,
    peer: &crate::configuration::domain::model::Peer,
) -> Result<TrustedPeer> {
    let mut trusted = peer.trusted.clone();
    if let Some(address) = &peer.settings.address {
        trusted.addresses = network.resolve(address).await?;
    }
    Ok(trusted)
}
