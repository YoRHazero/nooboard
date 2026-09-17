use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const PROTOCOL_VERSION: u16 = 3;
pub const MAX_CHUNK_BYTES: usize = 256 * 1024;
pub const MAX_TEXT_BYTES: usize = 1024 * 1024;
// JSON can expand each control byte into a six-byte escape.
const MAX_FRAME_BYTES: usize = MAX_TEXT_BYTES * 6 + 4096;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(deny_unknown_fields)]
pub struct MessageId {
    pub session: String,
    pub sequence: u64,
}
impl MessageId {
    pub(crate) fn valid(&self) -> bool {
        self.sequence != 0
            && self.session.len() == 32
            && self
                .session
                .bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Message {
    Hello {
        version: u16,
    },
    Device {
        device_name: String,
    },
    State {
        epoch: u64,
        accepting: bool,
    },
    Text {
        id: MessageId,
        target_epoch: u64,
        text: String,
    },
    Applied {
        id: MessageId,
    },
    Rejected {
        id: MessageId,
    },
    Offer {
        id: MessageId,
        target_epoch: u64,
        manifest: Manifest,
    },
    Accept {
        id: MessageId,
    },
    #[serde(skip)]
    Chunk {
        id: MessageId,
        file: u16,
        offset: u64,
        bytes: Vec<u8>,
    },
    ChunkAck {
        id: MessageId,
        file: u16,
        offset: u64,
    },
    Finish {
        id: MessageId,
    },
    Cancel {
        id: MessageId,
    },
    Outcome {
        id: MessageId,
        result: ContentResult,
        error: Option<TransferError>,
    },
    Ping,
    Pong,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ContentKind {
    Image,
    Files,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FileEntry {
    pub name: String,
    pub bytes: u64,
    pub sha256: [u8; 32],
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub kind: ContentKind,
    pub files: Vec<FileEntry>,
}
impl Manifest {
    pub fn bytes(&self) -> u64 {
        self.files.iter().map(|f| f.bytes).sum()
    }
    pub fn validate(&self) -> Result<()> {
        if self.files.is_empty()
            || self.files.len() > 256
            || self.files.iter().any(|f| {
                f.name.is_empty()
                    || f.name.len() > 1024
                    || f.name.contains('\0')
                    || f.bytes > 2 * 1024 * 1024 * 1024
            })
            || self
                .files
                .iter()
                .try_fold(0u64, |n, f| n.checked_add(f.bytes))
                .is_none_or(|n| n > 8 * 1024 * 1024 * 1024)
            || (self.kind == ContentKind::Image
                && (self.files.len() != 1 || self.files[0].bytes > 64 * 1024 * 1024))
        {
            Err(Error::Protocol)
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ContentResult {
    Applied,
    Saved,
    Rejected,
    Cancelled,
    Failed,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum TransferError {
    Denied,
    Directory,
    Unsupported,
    TooLarge,
    SourceChanged,
    Integrity,
    Io,
    Clipboard,
    Offline,
    Timeout,
    Busy,
    Cancelled,
    Protocol,
}
impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text {
                id,
                target_epoch,
                text,
            } => f
                .debug_struct("Text")
                .field("id", id)
                .field("target_epoch", target_epoch)
                .field("bytes", &text.len())
                .finish(),
            Self::Device { device_name } => f.debug_tuple("Device").field(device_name).finish(),
            Self::Hello { version } => f.debug_tuple("Hello").field(version).finish(),
            Self::State { epoch, accepting } => f
                .debug_tuple("State")
                .field(epoch)
                .field(accepting)
                .finish(),
            Self::Applied { id } => f.debug_tuple("Applied").field(id).finish(),
            Self::Rejected { id } => f.debug_tuple("Rejected").field(id).finish(),
            Self::Offer { id, manifest, .. } => f
                .debug_struct("Offer")
                .field("id", id)
                .field("kind", &manifest.kind)
                .field("files", &manifest.files.len())
                .finish(),
            Self::Chunk {
                id,
                file,
                offset,
                bytes,
            } => f
                .debug_struct("Chunk")
                .field("id", id)
                .field("file", file)
                .field("offset", offset)
                .field("bytes", &bytes.len())
                .finish(),
            Self::Accept { id } => f.debug_tuple("Accept").field(id).finish(),
            Self::ChunkAck { id, file, offset } => f
                .debug_tuple("ChunkAck")
                .field(id)
                .field(file)
                .field(offset)
                .finish(),
            Self::Finish { id } => f.debug_tuple("Finish").field(id).finish(),
            Self::Cancel { id } => f.debug_tuple("Cancel").field(id).finish(),
            Self::Outcome { id, result, error } => f
                .debug_tuple("Outcome")
                .field(id)
                .field(result)
                .field(error)
                .finish(),
            Self::Ping => f.write_str("Ping"),
            Self::Pong => f.write_str("Pong"),
        }
    }
}
pub fn valid_device_name(name: &str) -> bool {
    !name.trim().is_empty() && name.chars().count() <= 80 && !name.chars().any(char::is_control)
}
impl Message {
    fn validate(&self) -> Result<()> {
        match self {
            Self::Hello { version } if *version != PROTOCOL_VERSION => Err(Error::Protocol),
            Self::Text { id, text, .. }
                if !id.valid() || text.len() > MAX_TEXT_BYTES || text.contains('\0') =>
            {
                Err(Error::Protocol)
            }
            Self::Applied { id } | Self::Rejected { id } if !id.valid() => Err(Error::Protocol),
            Self::Offer { id, manifest, .. } => {
                if !id.valid() {
                    return Err(Error::Protocol);
                }
                manifest.validate()
            }
            Self::Accept { id }
            | Self::ChunkAck { id, .. }
            | Self::Finish { id }
            | Self::Cancel { id }
            | Self::Outcome { id, .. }
                if !id.valid() =>
            {
                Err(Error::Protocol)
            }
            Self::Chunk {
                id,
                file,
                bytes,
                offset,
            } if !id.valid()
                || *file >= 256
                || bytes.is_empty()
                || bytes.len() > MAX_CHUNK_BYTES
                || offset
                    .checked_add(bytes.len() as u64)
                    .is_none_or(|n| n > 2 * 1024 * 1024 * 1024) =>
            {
                Err(Error::Protocol)
            }
            Self::Device { device_name } if !valid_device_name(device_name) => Err(Error::Protocol),
            _ => Ok(()),
        }
    }
}
pub(crate) async fn write(writer: &mut (impl AsyncWrite + Unpin), message: &Message) -> Result<()> {
    message.validate()?;
    if let Message::Chunk {
        id,
        file,
        offset,
        bytes,
    } = message
    {
        writer.write_u32((51 + bytes.len()) as u32).await?;
        writer.write_u8(1).await?;
        writer.write_all(id.session.as_bytes()).await?;
        writer.write_u64(id.sequence).await?;
        writer.write_u16(*file).await?;
        writer.write_u64(*offset).await?;
        writer.write_all(bytes).await?;
        writer.flush().await?;
        return Ok(());
    }
    let bytes = serde_json::to_vec(message).map_err(|_| Error::Protocol)?;
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(Error::Protocol);
    }
    writer.write_u32((bytes.len() + 1) as u32).await?;
    writer.write_u8(0).await?;
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
    let tag = reader.read_u8().await?;
    if tag == 1 {
        if size <= 51 || size > 51 + MAX_CHUNK_BYTES {
            return Err(Error::Protocol);
        }
        let mut session = [0; 32];
        reader.read_exact(&mut session).await?;
        let id = MessageId {
            session: String::from_utf8(session.to_vec()).map_err(|_| Error::Protocol)?,
            sequence: reader.read_u64().await?,
        };
        let file = reader.read_u16().await?;
        let offset = reader.read_u64().await?;
        let mut bytes = vec![0; size - 51];
        reader.read_exact(&mut bytes).await?;
        let message = Message::Chunk {
            id,
            file,
            offset,
            bytes,
        };
        message.validate()?;
        return Ok(message);
    }
    if tag != 0 {
        return Err(Error::Protocol);
    }
    let mut bytes = vec![0; size - 1];
    reader.read_exact(&mut bytes).await?;
    let message: Message = serde_json::from_slice(&bytes).map_err(|_| Error::Protocol)?;
    message.validate()?;
    Ok(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn binary_chunks_roundtrip_without_json_expansion_and_are_bounded() {
        let id = MessageId {
            session: "a".repeat(32),
            sequence: 42,
        };
        let message = Message::Chunk {
            id: id.clone(),
            file: 2,
            offset: 17,
            bytes: vec![255; MAX_CHUNK_BYTES],
        };
        let mut buffer = Vec::new();
        write(&mut buffer, &message).await.unwrap();
        assert_eq!(buffer.len(), MAX_CHUNK_BYTES + 55);
        assert_eq!(read(&mut buffer.as_slice()).await.unwrap(), message);
        assert!(
            write(
                &mut Vec::new(),
                &Message::Chunk {
                    id,
                    file: 0,
                    offset: 0,
                    bytes: vec![0; MAX_CHUNK_BYTES + 1]
                }
            )
            .await
            .is_err()
        );
        buffer.pop();
        assert!(read(&mut buffer.as_slice()).await.is_err());
        assert!(Message::Hello { version: 2 }.validate().is_err());
    }
    #[tokio::test]
    async fn unicode_frame_roundtrip_and_body_redaction() {
        let message = Message::Text {
            id: MessageId {
                session: "a".repeat(32),
                sequence: 1,
            },
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
        assert!(Message::Hello { version: 1 }.validate().is_err());
        assert!(
            Message::Text {
                id: MessageId {
                    session: "a".repeat(32),
                    sequence: 1
                },
                target_epoch: 0,
                text: "a\0b".into()
            }
            .validate()
            .is_err()
        );
        assert!(read(&mut [0, 0, 0, 5, b'{'].as_slice()).await.is_err());
    }
}
