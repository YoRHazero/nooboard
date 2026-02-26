use std::net::SocketAddr;
use std::time::Duration;

use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_rustls::TlsStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::auth::{AuthCheck, ChallengeRegistry, SocketId, compute_auth_hash};
use crate::config::SyncConfig;
use crate::error::SyncError;
use crate::protocol::{HandshakePacket, Packet, require_handshake};
use crate::transport::{recv_packet, send_packet};

pub(super) async fn perform_client_handshake(
    config: &SyncConfig,
    peer_addr: SocketAddr,
    framed: &mut Framed<TlsStream<TcpStream>, LengthDelimitedCodec>,
    expected_node_id: Option<String>,
) -> Result<(String, String), SyncError> {
    send_packet(
        framed,
        &Packet::Handshake(HandshakePacket::Hello {
            protocol_version: config.protocol_version,
            node_id: config.noob_id.clone(),
            device_id: config.device_id.clone(),
        }),
    )
    .await?;

    let handshake_timeout = Duration::from_millis(config.handshake_timeout_ms);
    let first_packet = timeout(handshake_timeout, recv_handshake_only(framed))
        .await
        .map_err(|_| SyncError::HandshakeMessage("wait challenge timeout".to_string()))??;

    let mut peer_node_id = expected_node_id;
    let mut peer_device_id = None;
    let challenge = match first_packet {
        HandshakePacket::Hello {
            protocol_version,
            node_id,
            device_id,
        } => {
            if protocol_version != config.protocol_version {
                return Err(SyncError::HandshakeMessage(format!(
                    "protocol version mismatch: peer={protocol_version}, local={}",
                    config.protocol_version
                )));
            }
            peer_node_id = Some(node_id);
            peer_device_id = Some(device_id);
            timeout(handshake_timeout, recv_handshake_only(framed))
                .await
                .map_err(|_| SyncError::HandshakeMessage("wait challenge timeout".to_string()))??
        }
        packet => packet,
    };

    let HandshakePacket::Challenge { nonce } = challenge else {
        return Err(SyncError::HandshakeMessage(
            "expected Handshake::Challenge".to_string(),
        ));
    };

    let hash = compute_auth_hash(&config.token, &nonce);
    send_packet(
        framed,
        &Packet::Handshake(HandshakePacket::AuthResponse { hash }),
    )
    .await?;

    let auth_result = timeout(handshake_timeout, recv_handshake_only(framed))
        .await
        .map_err(|_| SyncError::HandshakeMessage("wait auth result timeout".to_string()))??;

    match auth_result {
        HandshakePacket::AuthResult { ok: true } => {
            let peer_node_id = peer_node_id.unwrap_or_else(|| format!("addr-{peer_addr}"));
            let peer_device_id = peer_device_id.unwrap_or_else(|| peer_node_id.clone());
            Ok((peer_node_id, peer_device_id))
        }
        HandshakePacket::AuthResult { ok: false } => Err(SyncError::HandshakeMessage(
            "auth rejected by peer".to_string(),
        )),
        _ => Err(SyncError::HandshakeMessage(
            "expected Handshake::AuthResult".to_string(),
        )),
    }
}

pub(super) async fn perform_server_handshake(
    config: &SyncConfig,
    socket_id: SocketId,
    challenge_registry: &ChallengeRegistry,
    framed: &mut Framed<TlsStream<TcpStream>, LengthDelimitedCodec>,
) -> Result<(String, String), SyncError> {
    let handshake_timeout = Duration::from_millis(config.handshake_timeout_ms);

    let hello = timeout(handshake_timeout, recv_handshake_only(framed))
        .await
        .map_err(|_| SyncError::HandshakeMessage("wait hello timeout".to_string()))??;

    let (protocol_version, peer_node_id, peer_device_id) = match hello {
        HandshakePacket::Hello {
            protocol_version,
            node_id,
            device_id,
        } => (protocol_version, node_id, device_id),
        _ => {
            return Err(SyncError::HandshakeMessage(
                "expected Handshake::Hello".to_string(),
            ));
        }
    };

    if protocol_version != config.protocol_version {
        return Err(SyncError::HandshakeMessage(format!(
            "protocol version mismatch: peer={protocol_version}, local={}",
            config.protocol_version
        )));
    }

    send_packet(
        framed,
        &Packet::Handshake(HandshakePacket::Hello {
            protocol_version: config.protocol_version,
            node_id: config.noob_id.clone(),
            device_id: config.device_id.clone(),
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
                &Packet::Handshake(HandshakePacket::AuthResult { ok: false }),
            )
            .await;
            return Err(SyncError::HandshakeMessage(
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
                &Packet::Handshake(HandshakePacket::AuthResult { ok: false }),
            )
            .await;
            return Err(SyncError::HandshakeMessage(
                "expected Handshake::AuthResponse".to_string(),
            ));
        }
    };

    let check = challenge_registry
        .verify_response(socket_id, &config.token, &hash)
        .await;

    match check {
        AuthCheck::Accepted => {
            send_packet(
                framed,
                &Packet::Handshake(HandshakePacket::AuthResult { ok: true }),
            )
            .await?;
            Ok((peer_node_id, peer_device_id))
        }
        AuthCheck::Rejected | AuthCheck::Timeout | AuthCheck::Missing => {
            let _ = send_packet(
                framed,
                &Packet::Handshake(HandshakePacket::AuthResult { ok: false }),
            )
            .await;
            Err(SyncError::HandshakeMessage(
                "authentication failed".to_string(),
            ))
        }
    }
}

async fn recv_handshake_only(
    framed: &mut Framed<TlsStream<TcpStream>, LengthDelimitedCodec>,
) -> Result<HandshakePacket, SyncError> {
    let packet = recv_packet(framed).await?;
    let packet =
        packet.ok_or_else(|| SyncError::HandshakeMessage("connection closed".to_string()))?;
    require_handshake(packet).map_err(|_| {
        SyncError::HandshakeMessage(
            "received non-handshake packet before authentication".to_string(),
        )
    })
}
