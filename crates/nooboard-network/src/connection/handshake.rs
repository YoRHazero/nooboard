use std::net::SocketAddr;
use std::time::Duration;

use tokio::time::timeout;

use crate::auth::{AuthCheck, ChallengeRegistry, SocketId, compute_auth_hash};
use crate::config::NetworkConfig;
use crate::errors::{ConnectionError, ProtocolError};
use crate::protocol::{ConnectionIntent, HandshakePacket, Packet, require_handshake};
use crate::transport::{NetworkFramed, recv_packet, send_packet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuthenticatedPeer {
    pub(crate) noob_id: String,
    pub(crate) device_id: String,
    pub(crate) intent: ConnectionIntent,
}

pub(crate) async fn perform_client_handshake(
    config: &NetworkConfig,
    peer_addr: SocketAddr,
    framed: &mut NetworkFramed,
    intent: ConnectionIntent,
) -> Result<AuthenticatedPeer, ConnectionError> {
    send_packet(
        framed,
        &Packet::Handshake(HandshakePacket::Hello {
            protocol_version: crate::protocol::PROTOCOL_VERSION,
            noob_id: config.identity.noob_id.clone(),
            device_id: config.identity.device_id.clone(),
            intent,
        }),
    )
    .await?;

    let handshake_timeout = Duration::from_millis(config.transport.handshake_timeout_ms);
    let first_packet = timeout(handshake_timeout, recv_handshake_only(framed))
        .await
        .map_err(|_| ConnectionError::State("wait challenge timeout".to_string()))??;

    let (peer, challenge) = match first_packet {
        HandshakePacket::Hello {
            protocol_version,
            noob_id,
            device_id,
            intent,
        } => {
            if protocol_version != crate::protocol::PROTOCOL_VERSION {
                return Err(ConnectionError::State(format!(
                    "protocol version mismatch: peer={protocol_version}, local={}",
                    crate::protocol::PROTOCOL_VERSION
                )));
            }
            let challenge = timeout(handshake_timeout, recv_handshake_only(framed))
                .await
                .map_err(|_| ConnectionError::State("wait challenge timeout".to_string()))??;
            (
                AuthenticatedPeer {
                    noob_id: if noob_id.is_empty() {
                        format!("addr-{peer_addr}")
                    } else {
                        noob_id
                    },
                    device_id,
                    intent,
                },
                challenge,
            )
        }
        packet => (
            AuthenticatedPeer {
                noob_id: format!("addr-{peer_addr}"),
                device_id: format!("addr-{peer_addr}"),
                intent,
            },
            packet,
        ),
    };

    let HandshakePacket::Challenge { nonce } = challenge else {
        return Err(ConnectionError::State(
            "expected Handshake::Challenge".to_string(),
        ));
    };

    let hash = compute_auth_hash(&config.auth.token, &nonce);
    send_packet(
        framed,
        &Packet::Handshake(HandshakePacket::AuthResponse { hash }),
    )
    .await?;

    let auth_result = timeout(handshake_timeout, recv_handshake_only(framed))
        .await
        .map_err(|_| ConnectionError::State("wait auth result timeout".to_string()))??;

    match auth_result {
        HandshakePacket::AuthAccepted => Ok(peer),
        HandshakePacket::AuthRejected { reason } => Err(ConnectionError::State(format!(
            "auth rejected by peer: {reason}"
        ))),
        _ => Err(ConnectionError::State(
            "expected Handshake::AuthAccepted/AuthRejected".to_string(),
        )),
    }
}

pub(crate) async fn perform_server_handshake(
    config: &NetworkConfig,
    socket_id: SocketId,
    challenge_registry: &ChallengeRegistry,
    framed: &mut NetworkFramed,
) -> Result<AuthenticatedPeer, ConnectionError> {
    let handshake_timeout = Duration::from_millis(config.transport.handshake_timeout_ms);

    let hello = timeout(handshake_timeout, recv_handshake_only(framed))
        .await
        .map_err(|_| ConnectionError::State("wait hello timeout".to_string()))??;

    let peer = match hello {
        HandshakePacket::Hello {
            protocol_version,
            noob_id,
            device_id,
            intent,
        } => {
            if protocol_version != crate::protocol::PROTOCOL_VERSION {
                return Err(ConnectionError::State(format!(
                    "protocol version mismatch: peer={protocol_version}, local={}",
                    crate::protocol::PROTOCOL_VERSION
                )));
            }
            AuthenticatedPeer {
                noob_id,
                device_id,
                intent,
            }
        }
        _ => {
            return Err(ConnectionError::State(
                "expected Handshake::Hello".to_string(),
            ));
        }
    };

    send_packet(
        framed,
        &Packet::Handshake(HandshakePacket::Hello {
            protocol_version: crate::protocol::PROTOCOL_VERSION,
            noob_id: config.identity.noob_id.clone(),
            device_id: config.identity.device_id.clone(),
            intent: ConnectionIntent::LanSync,
        }),
    )
    .await?;

    let nonce = challenge_registry
        .issue_challenge(socket_id, handshake_timeout)
        .await;

    if let Err(error) = send_packet(
        framed,
        &Packet::Handshake(HandshakePacket::Challenge { nonce }),
    )
    .await
    {
        challenge_registry.clear(socket_id).await;
        return Err(error.into());
    }

    let response = timeout(handshake_timeout, recv_handshake_only(framed)).await;
    let response = match response {
        Ok(Ok(packet)) => packet,
        Ok(Err(error)) => {
            challenge_registry.clear(socket_id).await;
            return Err(error);
        }
        Err(_) => {
            challenge_registry.clear(socket_id).await;
            let _ = send_packet(
                framed,
                &Packet::Handshake(HandshakePacket::AuthRejected {
                    reason: "wait auth response timeout".to_string(),
                }),
            )
            .await;
            return Err(ConnectionError::State(
                "wait auth response timeout".to_string(),
            ));
        }
    };

    let hash = match response {
        HandshakePacket::AuthResponse { hash } => hash,
        _ => {
            challenge_registry.clear(socket_id).await;
            let _ = send_packet(
                framed,
                &Packet::Handshake(HandshakePacket::AuthRejected {
                    reason: "expected Handshake::AuthResponse".to_string(),
                }),
            )
            .await;
            return Err(ConnectionError::State(
                "expected Handshake::AuthResponse".to_string(),
            ));
        }
    };

    let check = challenge_registry
        .verify_response(socket_id, &config.auth.token, &hash)
        .await;

    match check {
        AuthCheck::Accepted => {
            send_packet(framed, &Packet::Handshake(HandshakePacket::AuthAccepted)).await?;
            Ok(peer)
        }
        AuthCheck::Rejected | AuthCheck::Timeout | AuthCheck::Missing => {
            let _ = send_packet(
                framed,
                &Packet::Handshake(HandshakePacket::AuthRejected {
                    reason: "authentication failed".to_string(),
                }),
            )
            .await;
            Err(ConnectionError::State("authentication failed".to_string()))
        }
    }
}

async fn recv_handshake_only(
    framed: &mut NetworkFramed,
) -> Result<HandshakePacket, ConnectionError> {
    let packet = recv_packet(framed).await?;
    let packet = packet.ok_or_else(|| ConnectionError::State("connection closed".to_string()))?;
    require_handshake(packet).map_err(|_| {
        ConnectionError::Transport(crate::errors::TransportError::Protocol(
            ProtocolError::HandshakeRequired,
        ))
    })
}
