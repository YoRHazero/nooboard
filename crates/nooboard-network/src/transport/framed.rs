use bytes::Bytes;
use futures::stream::SplitSink;
use futures::{SinkExt, StreamExt};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::errors::TransportError;
use crate::protocol::{Packet, decode_packet, encode_packet};

use super::NetworkStream;

pub(crate) fn framed_with_max_packet(
    stream: NetworkStream,
    max_packet_size: usize,
) -> Framed<NetworkStream, LengthDelimitedCodec> {
    let codec = LengthDelimitedCodec::builder()
        .max_frame_length(max_packet_size)
        .new_codec();
    Framed::new(stream, codec)
}

pub(crate) async fn send_packet<S>(
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

pub(crate) async fn recv_packet<S>(
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

pub(crate) async fn send_packet_sink<S>(
    sink: &mut SplitSink<Framed<S, LengthDelimitedCodec>, Bytes>,
    packet: &Packet,
) -> Result<(), TransportError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let encoded = encode_packet(packet)?;
    sink.send(encoded.into())
        .await
        .map_err(TransportError::Io)?;
    Ok(())
}
