use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataPacket {
    ClipboardText {
        event_id: String,
        content: String,
    },
    FileStart {
        transfer_id: u32,
        file_name: String,
        file_size: u64,
        total_chunks: u32,
    },
    FileDecision {
        transfer_id: u32,
        accept: bool,
        reason: Option<String>,
    },
    FileChunk {
        transfer_id: u32,
        seq: u32,
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
    },
    FileEnd {
        transfer_id: u32,
        checksum: String,
    },
    FileCancel {
        transfer_id: u32,
    },
}
