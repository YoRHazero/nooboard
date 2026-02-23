use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{Sink, SinkExt, Stream, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{accept_async, connect_async};
use tracing::{debug, warn};

use crate::error::SyncError;
use crate::protocol::{HelloMessage, SyncEvent, WireMessage, decode_message, encode_message};

#[derive(Debug, Clone)]
pub struct TransportConfig {
    pub device_id: String,
    pub token: String,
    pub listen_addr: SocketAddr,
    pub peers: Vec<SocketAddr>,
}

pub struct TransportRuntime {
    pub peer_tx: mpsc::UnboundedSender<SocketAddr>,
    incoming_rx: mpsc::UnboundedReceiver<SyncEvent>,
    outbound_tx: broadcast::Sender<SyncEvent>,
    active_peers: Arc<Mutex<HashSet<String>>>,
}

struct PeerSlot {
    device_id: String,
    active_peers: Arc<Mutex<HashSet<String>>>,
}

impl Drop for PeerSlot {
    fn drop(&mut self) {
        if let Ok(mut active) = self.active_peers.lock() {
            active.remove(&self.device_id);
        }
    }
}

impl TransportRuntime {
    pub fn broadcast_event(&self, event: SyncEvent) -> Result<(), SyncError> {
        self.outbound_tx
            .send(event)
            .map(|_| ())
            .map_err(|_| SyncError::ChannelClosed)
    }

    pub async fn recv_event(&mut self) -> Option<SyncEvent> {
        self.incoming_rx.recv().await
    }

    pub fn connected_peer_count(&self) -> usize {
        self.active_peers
            .lock()
            .map(|active| active.len())
            .unwrap_or(0)
    }
}

pub async fn start_transport(config: TransportConfig) -> Result<TransportRuntime, SyncError> {
    let listener = TcpListener::bind(config.listen_addr).await?;
    let (outbound_tx, _) = broadcast::channel::<SyncEvent>(128);
    let (incoming_tx, incoming_rx) = mpsc::unbounded_channel::<SyncEvent>();
    let (peer_tx, mut peer_rx) = mpsc::unbounded_channel::<SocketAddr>();
    let active_peers = Arc::new(Mutex::new(HashSet::<String>::new()));

    let shared = Arc::new(config.clone());
    let listener_cfg = Arc::clone(&shared);
    let listener_outbound = outbound_tx.clone();
    let listener_incoming = incoming_tx.clone();
    let listener_active_peers = Arc::clone(&active_peers);
    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    let outbound = listener_outbound.subscribe();
                    let incoming = listener_incoming.clone();
                    let cfg = Arc::clone(&listener_cfg);
                    let active_peers = Arc::clone(&listener_active_peers);
                    tokio::spawn(async move {
                        if let Err(error) = handle_incoming_connection(
                            cfg.as_ref(),
                            stream,
                            addr,
                            outbound,
                            incoming,
                            active_peers,
                        )
                        .await
                        {
                            match error {
                                SyncError::DuplicatePeerConnection(device_id) => {
                                    debug!(
                                        "ignore duplicate incoming connection from {addr}, peer_device_id={device_id}"
                                    );
                                }
                                SyncError::DirectionRejected(device_id) => {
                                    debug!(
                                        "reject incoming connection by direction rule from {addr}, peer_device_id={device_id}, local_device_id={}",
                                        cfg.device_id
                                    );
                                }
                                SyncError::SelfConnection => {
                                    debug!("ignore self incoming connection from {addr}");
                                }
                                _ => warn!("incoming connection closed: {error}"),
                            }
                        }
                    });
                }
                Err(error) => {
                    warn!("accept failed: {error}");
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
        }
    });

    let connector_cfg = Arc::clone(&shared);
    let connector_outbound = outbound_tx.clone();
    let connector_incoming = incoming_tx.clone();
    let connector_active_peers = Arc::clone(&active_peers);
    tokio::spawn(async move {
        let mut known = HashSet::new();

        while let Some(peer_addr) = peer_rx.recv().await {
            if !known.insert(peer_addr) {
                continue;
            }

            let cfg = Arc::clone(&connector_cfg);
            let outbound = connector_outbound.clone();
            let incoming = connector_incoming.clone();
            let active_peers = Arc::clone(&connector_active_peers);
            tokio::spawn(async move {
                loop {
                    let outbound_rx = outbound.subscribe();
                    match connect_and_run(
                        cfg.as_ref(),
                        peer_addr,
                        outbound_rx,
                        incoming.clone(),
                        Arc::clone(&active_peers),
                    )
                    .await
                    {
                        Ok(()) => tokio::time::sleep(Duration::from_millis(500)).await,
                        Err(SyncError::DuplicatePeerConnection(device_id)) => {
                            debug!(
                                "skip duplicate connection to {peer_addr}, peer_device_id={device_id}"
                            );
                            tokio::time::sleep(Duration::from_secs(5)).await;
                        }
                        Err(SyncError::DirectionRejected(device_id)) => {
                            debug!(
                                "disable outbound dial to {peer_addr}, peer_device_id={device_id}, local_device_id={}",
                                cfg.device_id
                            );
                            break;
                        }
                        Err(SyncError::SelfConnection) => {
                            debug!("ignore self outbound dial to {peer_addr}");
                            break;
                        }
                        Err(error) => {
                            warn!("connect {peer_addr} failed: {error}");
                            tokio::time::sleep(Duration::from_secs(2)).await;
                        }
                    }
                }
            });
        }
    });

    Ok(TransportRuntime {
        incoming_rx,
        peer_tx,
        outbound_tx,
        active_peers,
    })
}

async fn handle_incoming_connection(
    config: &TransportConfig,
    stream: tokio::net::TcpStream,
    addr: SocketAddr,
    outbound_rx: broadcast::Receiver<SyncEvent>,
    incoming_tx: mpsc::UnboundedSender<SyncEvent>,
    active_peers: Arc<Mutex<HashSet<String>>>,
) -> Result<(), SyncError> {
    let ws_stream = accept_async(stream).await?;
    let (mut writer, mut reader) = ws_stream.split();

    let first = reader.next().await.ok_or(SyncError::ChannelClosed)??;
    let hello = parse_hello(first, &config.token)?;
    let peer_device_id = hello.device_id.clone();
    let ack = WireMessage::Hello(HelloMessage::new(&config.device_id, &config.token));
    writer
        .send(Message::Text(encode_message(&ack)?.into()))
        .await?;

    if peer_device_id == config.device_id {
        graceful_close(&mut writer).await;
        return Err(SyncError::SelfConnection);
    }
    if !should_accept_incoming(&config.device_id, &peer_device_id) {
        graceful_close(&mut writer).await;
        return Err(SyncError::DirectionRejected(peer_device_id));
    }
    let _peer_slot = match reserve_peer_slot(&active_peers, &peer_device_id) {
        Some(slot) => slot,
        None => {
            graceful_close(&mut writer).await;
            return Err(SyncError::DuplicatePeerConnection(peer_device_id));
        }
    };
    debug!("accepted peer {} ({addr})", peer_device_id);

    run_connection_loop(outbound_rx, incoming_tx, reader, writer).await
}

async fn connect_and_run(
    config: &TransportConfig,
    peer_addr: SocketAddr,
    outbound_rx: broadcast::Receiver<SyncEvent>,
    incoming_tx: mpsc::UnboundedSender<SyncEvent>,
    active_peers: Arc<Mutex<HashSet<String>>>,
) -> Result<(), SyncError> {
    let request_url = format!("ws://{peer_addr}/sync");
    let (ws_stream, _) = connect_async(request_url).await?;
    let (mut writer, mut reader) = ws_stream.split();

    let hello = WireMessage::Hello(HelloMessage::new(&config.device_id, &config.token));
    writer
        .send(Message::Text(encode_message(&hello)?.into()))
        .await?;

    let first = reader.next().await.ok_or(SyncError::ChannelClosed)??;
    let ack = parse_hello(first, &config.token)?;
    let peer_device_id = ack.device_id;
    if peer_device_id == config.device_id {
        graceful_close(&mut writer).await;
        return Err(SyncError::SelfConnection);
    }
    if !should_initiate_outbound(&config.device_id, &peer_device_id) {
        graceful_close(&mut writer).await;
        return Err(SyncError::DirectionRejected(peer_device_id));
    }
    let _peer_slot = match reserve_peer_slot(&active_peers, &peer_device_id) {
        Some(slot) => slot,
        None => {
            graceful_close(&mut writer).await;
            return Err(SyncError::DuplicatePeerConnection(peer_device_id));
        }
    };
    debug!("connected peer {}", peer_device_id);

    run_connection_loop(outbound_rx, incoming_tx, reader, writer).await
}

fn reserve_peer_slot(
    active_peers: &Arc<Mutex<HashSet<String>>>,
    device_id: &str,
) -> Option<PeerSlot> {
    let mut active = active_peers.lock().ok()?;
    if !active.insert(device_id.to_string()) {
        return None;
    }

    Some(PeerSlot {
        device_id: device_id.to_string(),
        active_peers: Arc::clone(active_peers),
    })
}

fn should_initiate_outbound(local_device_id: &str, peer_device_id: &str) -> bool {
    local_device_id < peer_device_id
}

fn should_accept_incoming(local_device_id: &str, peer_device_id: &str) -> bool {
    local_device_id > peer_device_id
}

async fn graceful_close<S>(writer: &mut S)
where
    S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    let _ = writer.send(Message::Close(None)).await;
}

fn parse_hello(message: Message, expected_token: &str) -> Result<HelloMessage, SyncError> {
    let Message::Text(text) = message else {
        return Err(SyncError::Protocol("expected hello text frame".to_string()));
    };
    match decode_message(text.as_ref())? {
        WireMessage::Hello(hello) => {
            if hello.token != expected_token {
                return Err(SyncError::AuthenticationFailed);
            }
            Ok(hello)
        }
        _ => Err(SyncError::Protocol(
            "expected hello as first message".to_string(),
        )),
    }
}

async fn run_connection_loop<
    ReaderStream: Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
    WriterSink: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
>(
    mut outbound_rx: broadcast::Receiver<SyncEvent>,
    incoming_tx: mpsc::UnboundedSender<SyncEvent>,
    mut reader: ReaderStream,
    mut writer: WriterSink,
) -> Result<(), SyncError> {
    loop {
        tokio::select! {
            received = reader.next() => {
                match received {
                    Some(Ok(Message::Text(text))) => {
                        if let WireMessage::Event(event) = decode_message(text.as_ref())? {
                            let _ = incoming_tx.send(event);
                        }
                    }
                    Some(Ok(Message::Binary(_))) => {}
                    Some(Ok(Message::Ping(payload))) => {
                        writer.send(Message::Pong(payload)).await?;
                    }
                    Some(Ok(Message::Pong(_))) => {}
                    Some(Ok(Message::Close(_))) => return Ok(()),
                    Some(Ok(Message::Frame(_))) => {}
                    Some(Err(error)) => return Err(SyncError::WebSocket(error)),
                    None => return Ok(()),
                }
            }
            outbound = outbound_rx.recv() => {
                match outbound {
                    Ok(event) => {
                        let payload = WireMessage::Event(event);
                        writer.send(Message::Text(encode_message(&payload)?.into())).await?;
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        warn!("outbound lagged by {skipped} messages");
                    }
                    Err(broadcast::error::RecvError::Closed) => return Ok(()),
                }
            }
        }
    }
}
