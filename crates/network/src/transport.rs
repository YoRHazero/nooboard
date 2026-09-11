use crate::{Error, Identity, Message, PROTOCOL_VERSION, Result, protocol};
use rustls::{
    ClientConfig, RootCertStore, ServerConfig,
    pki_types::{CertificateDer, ServerName},
};
use std::{sync::Arc, time::Duration};
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

/// Trust is configured with one explicitly confirmed peer certificate.
/// All chain and CertificateVerify checks use rustls' built-in WebPKI verifiers.
#[derive(Clone)]
pub struct TlsConfig {
    client: Arc<ClientConfig>,
    server: Arc<ServerConfig>,
    expected_peer: Vec<u8>,
}
impl TlsConfig {
    pub fn new(identity: &Identity, trusted_certificate: &[u8]) -> Result<Self> {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let mut roots = RootCertStore::empty();
        roots.add(CertificateDer::from(trusted_certificate.to_vec()))?;
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
        client.alpn_protocols = vec![b"nooboard/1".to_vec()];
        server.alpn_protocols = client.alpn_protocols.clone();
        // Sessions are always freshly authenticated; unpairing cannot leave resumed sessions.
        client.resumption = rustls::client::Resumption::disabled();
        server.send_tls13_tickets = 0;
        Ok(Self {
            client: Arc::new(client),
            server: Arc::new(server),
            expected_peer: trusted_certificate.to_vec(),
        })
    }
    pub async fn connect(&self, address: &str) -> Result<Connection> {
        tokio::time::timeout(HANDSHAKE_TIMEOUT, async {
            let tcp = TcpStream::connect(address).await?;
            tcp.set_nodelay(true)?;
            let name = ServerName::try_from("nooboard.local").map_err(|_| Error::Identity)?;
            let tls = TlsConnector::from(self.client.clone())
                .connect(name, tcp)
                .await?;
            self.finish(TlsStream::Client(tls)).await
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
        if state.alpn_protocol() != Some(b"nooboard/1".as_slice())
            || state
                .peer_certificates()
                .and_then(|c| c.first())
                .map(|c| c.as_ref())
                != Some(self.expected_peer.as_slice())
        {
            return Err(Error::Identity);
        }
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
            writer,
            incoming,
            task,
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
    writer: WriteHalf<TlsStream<TcpStream>>,
    incoming: mpsc::Receiver<Result<Message>>,
    task: JoinHandle<()>,
}
impl Connection {
    /// Cancel-safe: the framing reader is owned by its dedicated task.
    pub async fn receive(&mut self) -> Result<Message> {
        self.incoming.recv().await.ok_or(Error::Closed)?
    }
    /// A timeout or cancellation invalidates the stream. Drop it before reconnecting.
    pub async fn send(&mut self, message: &Message) -> Result<()> {
        tokio::time::timeout(WRITE_TIMEOUT, protocol::write(&mut self.writer, message))
            .await
            .map_err(|_| Error::Timeout)?
    }
}
impl Drop for Connection {
    fn drop(&mut self) {
        self.task.abort();
    }
}
