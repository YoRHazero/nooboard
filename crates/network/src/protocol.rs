use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const PROTOCOL_VERSION: u16 = 1;
pub const MAX_TEXT_BYTES: usize = 1024 * 1024;
// JSON can expand each control byte into a six-byte escape.
const MAX_FRAME_BYTES: usize = MAX_TEXT_BYTES * 6 + 4096;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Message {
    Hello {
        version: u16,
    },
    State {
        epoch: u64,
        accepting: bool,
    },
    Text {
        sequence: u64,
        target_epoch: u64,
        text: String,
    },
    Applied {
        sequence: u64,
    },
    Rejected {
        sequence: u64,
    },
    Ping,
    Pong,
}
impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text {
                sequence,
                target_epoch,
                text,
            } => f
                .debug_struct("Text")
                .field("sequence", sequence)
                .field("target_epoch", target_epoch)
                .field("bytes", &text.len())
                .finish(),
            Self::Hello { version } => f.debug_tuple("Hello").field(version).finish(),
            Self::State { epoch, accepting } => f
                .debug_tuple("State")
                .field(epoch)
                .field(accepting)
                .finish(),
            Self::Applied { sequence } => f.debug_tuple("Applied").field(sequence).finish(),
            Self::Rejected { sequence } => f.debug_tuple("Rejected").field(sequence).finish(),
            Self::Ping => f.write_str("Ping"),
            Self::Pong => f.write_str("Pong"),
        }
    }
}
impl Message {
    fn validate(&self) -> Result<()> {
        match self {
            Self::Hello { version } if *version != PROTOCOL_VERSION => Err(Error::Protocol),
            Self::Text { sequence, text, .. }
                if *sequence == 0 || text.len() > MAX_TEXT_BYTES || text.contains('\0') =>
            {
                Err(Error::Protocol)
            }
            _ => Ok(()),
        }
    }
}
pub(crate) async fn write(writer: &mut (impl AsyncWrite + Unpin), message: &Message) -> Result<()> {
    message.validate()?;
    let bytes = serde_json::to_vec(message).map_err(|_| Error::Protocol)?;
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(Error::Protocol);
    }
    writer.write_u32(bytes.len() as u32).await?;
    writer.write_all(&bytes).await?;
    writer.flush().await?;
    Ok(())
}
/// The caller must close the connection if this future is cancelled.
pub(crate) async fn read(reader: &mut (impl AsyncRead + Unpin)) -> Result<Message> {
    let size = reader.read_u32().await? as usize;
    if size == 0 || size > MAX_FRAME_BYTES {
        return Err(Error::Protocol);
    }
    let mut bytes = vec![0; size];
    reader.read_exact(&mut bytes).await?;
    let message: Message = serde_json::from_slice(&bytes).map_err(|_| Error::Protocol)?;
    message.validate()?;
    Ok(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn unicode_frame_roundtrip_and_body_redaction() {
        let message = Message::Text {
            sequence: 1,
            target_epoch: 2,
            text: " 秘密🦀\r\n ".into(),
        };
        let mut buffer = Vec::new();
        write(&mut buffer, &message).await.unwrap();
        assert_eq!(read(&mut buffer.as_slice()).await.unwrap(), message);
        assert!(!format!("{message:?}").contains("秘密"));
    }
    #[tokio::test]
    async fn rejects_oversize_header_without_waiting_for_body() {
        let bytes = ((MAX_FRAME_BYTES + 1) as u32).to_be_bytes();
        assert!(matches!(
            read(&mut bytes.as_slice()).await,
            Err(Error::Protocol)
        ));
    }
    #[tokio::test]
    async fn rejects_version_null_and_truncated_frames() {
        assert!(Message::Hello { version: 2 }.validate().is_err());
        assert!(
            Message::Text {
                sequence: 1,
                target_epoch: 0,
                text: "a\0b".into()
            }
            .validate()
            .is_err()
        );
        assert!(read(&mut [0, 0, 0, 5, b'{'].as_slice()).await.is_err());
    }
}
