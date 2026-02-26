use std::sync::Arc;

use bytes::Bytes;
use futures::stream::SplitSink;
use futures::{SinkExt, StreamExt};
use rcgen::generate_simple_self_signed;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{
    ClientConfig, DigitallySignedStruct, Error as RustlsError, ServerConfig, SignatureScheme,
};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_rustls::{TlsAcceptor, TlsConnector, TlsStream};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::error::TransportError;
use crate::protocol::{Packet, decode_packet, encode_packet};

#[derive(Clone)]
pub struct TlsContext {
    acceptor: TlsAcceptor,
    connector: TlsConnector,
}

impl TlsContext {
    pub fn ephemeral() -> Result<Self, TransportError> {
        let generated = generate_simple_self_signed(vec!["nooboard.local".to_string()])
            .map_err(|error| std::io::Error::other(error.to_string()))?;

        let cert_der = generated.cert.der().clone();
        let key_der = generated.signing_key.serialize_der();
        let key = PrivateKeyDer::from(PrivatePkcs8KeyDer::from(key_der));

        let server_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert_der], key)?;

        let client_config = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoCertificateVerification))
            .with_no_client_auth();

        Ok(Self {
            acceptor: TlsAcceptor::from(Arc::new(server_config)),
            connector: TlsConnector::from(Arc::new(client_config)),
        })
    }

    pub async fn accept(&self, stream: TcpStream) -> Result<TlsStream<TcpStream>, TransportError> {
        let stream = self.acceptor.accept(stream).await?;
        Ok(TlsStream::Server(stream))
    }

    pub async fn connect(
        &self,
        stream: TcpStream,
        server_name: &str,
    ) -> Result<TlsStream<TcpStream>, TransportError> {
        let server_name = ServerName::try_from(server_name.to_string())
            .map_err(|_| TransportError::InvalidServerName(server_name.to_string()))?;
        let stream = self.connector.connect(server_name, stream).await?;
        Ok(TlsStream::Client(stream))
    }
}

#[derive(Debug)]
struct NoCertificateVerification;

impl ServerCertVerifier for NoCertificateVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, RustlsError> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::RSA_PSS_SHA256,
        ]
    }
}

pub fn framed_with_max_packet(
    stream: TlsStream<TcpStream>,
    max_packet_size: usize,
) -> Framed<TlsStream<TcpStream>, LengthDelimitedCodec> {
    let codec = LengthDelimitedCodec::builder()
        .max_frame_length(max_packet_size)
        .new_codec();
    Framed::new(stream, codec)
}

pub async fn send_packet<S>(
    framed: &mut Framed<S, LengthDelimitedCodec>,
    packet: &Packet,
) -> Result<(), TransportError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let encoded = encode_packet(packet)?;
    framed.send(encoded.into()).await?;
    Ok(())
}

pub async fn recv_packet<S>(
    framed: &mut Framed<S, LengthDelimitedCodec>,
) -> Result<Option<Packet>, TransportError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    match framed.next().await {
        Some(Ok(bytes)) => Ok(Some(decode_packet(&bytes)?)),
        Some(Err(error)) => Err(TransportError::Io(error)),
        None => Ok(None),
    }
}

pub async fn send_packet_sink<S>(
    sink: &mut SplitSink<Framed<S, LengthDelimitedCodec>, Bytes>,
    packet: &Packet,
) -> Result<(), TransportError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let encoded = encode_packet(packet)?;
    sink.send(encoded.into())
        .await
        .map_err(|e| TransportError::Io(e))?;
    Ok(())
}
