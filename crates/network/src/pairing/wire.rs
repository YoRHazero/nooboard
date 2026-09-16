use super::{Contact, Error, Result};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hello {
    pub version: u8,
    pub nonce: String,
    pub contact: Contact,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "data", deny_unknown_fields)]
pub enum Packet {
    Hello(Hello),
    Waiting,
    Ready(Hello),
    Pake(Vec<u8>),
    Sealed(Vec<u8>),
    Retry(u8),
    Reject,
}
const MAX_FRAME: usize = 48 * 1024;
pub async fn read(stream: &mut TcpStream) -> Result<Packet> {
    let size = stream.read_u32().await.map_err(|_| Error::Disconnected)? as usize;
    if size == 0 || size > MAX_FRAME {
        return Err(Error::Protocol);
    }
    let mut data = vec![0; size];
    stream
        .read_exact(&mut data)
        .await
        .map_err(|_| Error::Disconnected)?;
    serde_json::from_slice(&data).map_err(|_| Error::Protocol)
}
pub async fn write(stream: &mut TcpStream, packet: &Packet) -> Result<()> {
    let data = serde_json::to_vec(packet).map_err(|_| Error::Protocol)?;
    if data.len() > MAX_FRAME {
        return Err(Error::Protocol);
    }
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        stream.write_u32(data.len() as u32).await?;
        stream.write_all(&data).await
    })
    .await
    .map_err(|_| Error::Timeout)?
    .map_err(|_| Error::Disconnected)
}
pub fn binding(hello: &Hello) -> Result<Vec<u8>> {
    serde_json::to_vec(hello).map_err(|_| Error::Protocol)
}
