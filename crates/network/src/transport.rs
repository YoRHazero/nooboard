use crate::{Error, Identity, Message, PROTOCOL_VERSION, Result, protocol};
use rustls::{
    ClientConfig, RootCertStore, ServerConfig,
    pki_types::{CertificateDer, ServerName},
};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use tokio::{
    io::{ReadHalf, WriteHalf},
    net::TcpStream,
    sync::mpsc,
    task::JoinHandle,
};
use tokio_rustls::{TlsAcceptor, TlsConnector, TlsStream};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);
const READ_TIMEOUT: Duration = Duration::from_secs(20);

/// Only explicitly confirmed certificates may establish business connections.
/// All chain and CertificateVerify checks use rustls' built-in WebPKI verifiers.
#[derive(Clone)]
pub struct TlsConfig {
    client: Arc<ClientConfig>,
    server: Arc<ServerConfig>,
    peers: BTreeMap<String, Vec<u8>>,
}
impl TlsConfig {
    pub fn new(identity: &Identity, trusted_certificate: &[u8]) -> Result<Self> {
        Self::with_peers(identity, &[trusted_certificate.to_vec()])
    }
    pub fn with_peers(identity: &Identity, certificates: &[Vec<u8>]) -> Result<Self> {
        let mut peers = BTreeMap::new();
        for certificate in certificates {
            let id = crate::noob_id(certificate)?;
            if peers.insert(id, certificate.clone()).is_some() {
                return Err(Error::Identity);
            }
        }
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let mut roots = RootCertStore::empty();
        for certificate in certificates {
            roots.add(CertificateDer::from(certificate.clone()))?;
        }
        let mut client = ClientConfig::builder_with_provider(provider.clone())
            .with_protocol_versions(&[&rustls::version::TLS13])?
            .with_root_certificates(roots.clone())
            .with_client_auth_cert(vec![identity.cert.clone()], identity.key.clone_key())?;
        let verifier = rustls::server::WebPkiClientVerifier::builder_with_provider(
            Arc::new(roots),
            provider.clone(),
        )
        .build()
        .map_err(|_| Error::Identity)?;
        let mut server = ServerConfig::builder_with_provider(provider)
            .with_protocol_versions(&[&rustls::version::TLS13])?
            .with_client_cert_verifier(verifier)
            .with_single_cert(vec![identity.cert.clone()], identity.key.clone_key())?;
        client.alpn_protocols = vec![b"nooboard/3".to_vec()];
        server.alpn_protocols = client.alpn_protocols.clone();
        // Sessions are always freshly authenticated; unpairing cannot leave resumed sessions.
        client.resumption = rustls::client::Resumption::disabled();
        server.send_tls13_tickets = 0;
        Ok(Self {
            client: Arc::new(client),
            server: Arc::new(server),
            peers,
        })
    }
    pub async fn connect(&self, address: &str) -> Result<Connection> {
        if self.peers.len() != 1 {
            return Err(Error::Identity);
        }
        self.connect_peer(address, self.peers.keys().next().expect("one peer"))
            .await
    }
    pub async fn connect_peer(&self, address: &str, expected_id: &str) -> Result<Connection> {
        if !self.peers.contains_key(expected_id) {
            return Err(Error::Identity);
        }
        tokio::time::timeout(HANDSHAKE_TIMEOUT, async {
            let tcp = TcpStream::connect(address).await?;
            tcp.set_nodelay(true)?;
            let name = ServerName::try_from("nooboard.local").map_err(|_| Error::Identity)?;
            let tls = TlsConnector::from(self.client.clone())
                .connect(name, tcp)
                .await?;
            let connection = self.finish(TlsStream::Client(tls)).await?;
            if connection.peer_id != expected_id {
                return Err(Error::Identity);
            }
            Ok(connection)
        })
        .await
        .map_err(|_| Error::Timeout)?
    }
    pub async fn accept(&self, tcp: TcpStream) -> Result<Connection> {
        tokio::time::timeout(HANDSHAKE_TIMEOUT, async {
            tcp.set_nodelay(true)?;
            let tls = TlsAcceptor::from(self.server.clone()).accept(tcp).await?;
            self.finish(TlsStream::Server(tls)).await
        })
        .await
        .map_err(|_| Error::Timeout)?
    }
    async fn finish(&self, mut stream: TlsStream<TcpStream>) -> Result<Connection> {
        let state = stream.get_ref().1;
        let certificate = state
            .peer_certificates()
            .and_then(|c| c.first())
            .ok_or(Error::Identity)?;
        let peer_id = crate::noob_id(certificate)?;
        if state.alpn_protocol() != Some(b"nooboard/3".as_slice())
            || self.peers.get(&peer_id).map(Vec::as_slice) != Some(certificate.as_ref())
        {
            return Err(Error::Identity);
        }
        let peer_fingerprint = crate::fingerprint(certificate);
        protocol::write(
            &mut stream,
            &Message::Hello {
                version: PROTOCOL_VERSION,
            },
        )
        .await?;
        if protocol::read(&mut stream).await?
            != (Message::Hello {
                version: PROTOCOL_VERSION,
            })
        {
            return Err(Error::Protocol);
        }
        let (reader, writer) = tokio::io::split(stream);
        let (sender, incoming) = mpsc::channel(16);
        let task = tokio::spawn(read_messages(reader, sender));
        Ok(Connection {
            sender: ConnectionSender { writer },
            receiver: ConnectionReceiver { incoming, task },
            peer_id,
            peer_fingerprint,
        })
    }
}
async fn read_messages(
    mut reader: ReadHalf<TlsStream<TcpStream>>,
    sender: mpsc::Sender<Result<Message>>,
) {
    loop {
        // The dedicated reader is never cancelled by unrelated core events.
        let result = tokio::time::timeout(READ_TIMEOUT, protocol::read(&mut reader))
            .await
            .map_err(|_| Error::Timeout)
            .and_then(|r| r);
        let failed = result.is_err();
        if sender.send(result).await.is_err() || failed {
            return;
        }
    }
}
pub struct Connection {
    sender: ConnectionSender,
    receiver: ConnectionReceiver,
    peer_id: String,
    peer_fingerprint: String,
}
impl Connection {
    pub fn peer_id(&self) -> &str {
        &self.peer_id
    }
    pub fn peer_fingerprint(&self) -> &str {
        &self.peer_fingerprint
    }
    pub fn into_split(self) -> (ConnectionSender, ConnectionReceiver) {
        (self.sender, self.receiver)
    }
    pub async fn receive(&mut self) -> Result<Message> {
        self.receiver.receive().await
    }
    pub async fn send(&mut self, message: &Message) -> Result<()> {
        self.sender.send(message).await
    }
}
pub struct ConnectionSender {
    writer: WriteHalf<TlsStream<TcpStream>>,
}
impl ConnectionSender {
    /// A timeout or cancellation invalidates the stream. Drop it before reconnecting.
    pub async fn send(&mut self, message: &Message) -> Result<()> {
        tokio::time::timeout(WRITE_TIMEOUT, protocol::write(&mut self.writer, message))
            .await
            .map_err(|_| Error::Timeout)?
    }
}
pub struct ConnectionReceiver {
    incoming: mpsc::Receiver<Result<Message>>,
    task: JoinHandle<()>,
}
impl ConnectionReceiver {
    /// Cancel-safe: the framing reader is owned by its dedicated task.
    pub async fn receive(&mut self) -> Result<Message> {
        self.incoming.recv().await.ok_or(Error::Closed)?
    }
}
impl Drop for ConnectionReceiver {
    fn drop(&mut self) {
        self.task.abort();
    }
}
