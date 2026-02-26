use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_rustls::TlsStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::auth::{ChallengeRegistry, SocketId};
use crate::config::SyncConfig;
use crate::error::SyncError;
use crate::transport::{TlsContext, framed_with_max_packet};

use super::candidates::ConnectTarget;
use super::handshake::{perform_client_handshake, perform_server_handshake};
use super::peers::{EngineControl, PeerRegistry};
use super::policy::DedupeDecision;

pub(super) fn schedule_connect_attempts(
    config: &SyncConfig,
    registry: &mut PeerRegistry,
    tls: &TlsContext,
    control_tx: &mpsc::Sender<EngineControl>,
) {
    let targets = registry.connect_targets(&config.manual_peers);

    for target in targets {
        if registry.should_skip_target(&target.addr) {
            continue;
        }

        registry.mark_connecting(target.addr);

        let config = config.clone();
        let tls = tls.clone();
        let control_tx = control_tx.clone();

        tokio::spawn(async move {
            let result = connect_outbound_peer(&config, &tls, target.clone()).await;
            match result {
                Ok((peer_node_id, peer_device_id, framed)) => {
                    let _ = control_tx
                        .send(EngineControl::Connected {
                            peer_node_id,
                            peer_device_id,
                            addr: target.addr,
                            outbound: true,
                            framed,
                        })
                        .await;
                }
                Err(error) => {
                    let _ = control_tx
                        .send(EngineControl::ConnectFailed {
                            addr: target.addr,
                            error,
                        })
                        .await;
                }
            }

            let _ = control_tx
                .send(EngineControl::ConnectAttemptFinished { addr: target.addr })
                .await;
        });
    }
}

pub(super) async fn connect_outbound_peer(
    config: &SyncConfig,
    tls: &TlsContext,
    target: ConnectTarget,
) -> Result<
    (
        String,
        String,
        Framed<TlsStream<TcpStream>, LengthDelimitedCodec>,
    ),
    SyncError,
> {
    let stream = timeout(
        Duration::from_millis(config.connect_timeout_ms),
        TcpStream::connect(target.addr),
    )
    .await
    .map_err(|_| SyncError::HandshakeMessage("connect timeout".to_string()))??;

    let tls_stream = tls.connect(stream, "nooboard.local").await?;
    let mut framed = framed_with_max_packet(tls_stream, config.max_packet_size);

    let (peer_node_id, peer_device_id) =
        perform_client_handshake(config, target.addr, &mut framed, target.expected_node_id).await?;
    Ok((peer_node_id, peer_device_id, framed))
}

pub(super) async fn accept_inbound_peer(
    stream: TcpStream,
    _addr: SocketAddr,
    socket_id: SocketId,
    config: &SyncConfig,
    tls: &TlsContext,
    challenge_registry: Arc<ChallengeRegistry>,
) -> Result<
    (
        String,
        String,
        Framed<TlsStream<TcpStream>, LengthDelimitedCodec>,
    ),
    SyncError,
> {
    let tls_stream = tls.accept(stream).await?;
    let mut framed = framed_with_max_packet(tls_stream, config.max_packet_size);

    let (peer_node_id, peer_device_id) =
        perform_server_handshake(config, socket_id, &challenge_registry, &mut framed).await?;
    Ok((peer_node_id, peer_device_id, framed))
}

pub(super) fn connection_direction_allowed(outbound: bool, decision: DedupeDecision) -> bool {
    match decision {
        DedupeDecision::ConnectOut => outbound,
        DedupeDecision::WaitInbound => !outbound,
        DedupeDecision::RejectConflict => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::policy::dedupe_decision;

    #[test]
    fn dedupe_direction_follows_small_connect_large_rule() {
        assert!(connection_direction_allowed(
            true,
            dedupe_decision("a", "b")
        ));
        assert!(!connection_direction_allowed(
            false,
            dedupe_decision("a", "b")
        ));
        assert!(connection_direction_allowed(
            false,
            dedupe_decision("z", "b")
        ));
    }
}
