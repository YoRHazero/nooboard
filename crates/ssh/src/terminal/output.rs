use crate::{Error, Result, TerminalChunk, TerminalReplay};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
};
use tokio::sync::broadcast;

#[derive(Default)]
struct Buffer {
    bytes: VecDeque<u8>,
    end: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replay_reports_eviction_and_keeps_sessions_independent() {
        let output = Output::new(5);
        output.register("one");
        output.register("two");
        output.push("one", b"1234");
        output.push("one", b"5678");
        output.push("two", b"other");
        let replay = output.replay("one", 0).unwrap();
        assert!(replay.gap);
        assert_eq!((replay.start, replay.end), (3, 8));
        assert_eq!(replay.bytes, b"45678");
        assert_eq!(output.replay("one", 6).unwrap().bytes, b"78");
        assert_eq!(output.replay("two", 0).unwrap().bytes, b"other");
        output.remove("one");
        assert!(output.replay("one", 0).is_err());
    }
}
#[derive(Clone)]
pub(crate) struct Output {
    buffers: Arc<Mutex<HashMap<String, Buffer>>>,
    events: broadcast::Sender<TerminalChunk>,
    limit: usize,
}
impl Output {
    pub fn new(limit: usize) -> Self {
        Self {
            buffers: Arc::default(),
            events: broadcast::channel(128).0,
            limit,
        }
    }
    pub fn register(&self, id: &str) {
        self.buffers
            .lock()
            .expect("output poisoned")
            .insert(id.into(), Buffer::default());
    }
    pub fn remove(&self, id: &str) {
        self.buffers.lock().expect("output poisoned").remove(id);
    }
    pub fn subscribe(&self) -> broadcast::Receiver<TerminalChunk> {
        self.events.subscribe()
    }
    pub fn push(&self, id: &str, bytes: &[u8]) {
        // Bound the broadcast channel by chunk size as well as message count.
        for bytes in bytes.chunks(16 * 1024) {
            let mut buffers = self.buffers.lock().expect("output poisoned");
            let Some(buffer) = buffers.get_mut(id) else {
                return;
            };
            let offset = buffer.end;
            buffer.end += bytes.len() as u64;
            buffer.bytes.extend(bytes);
            let excess = buffer.bytes.len().saturating_sub(self.limit);
            buffer.bytes.drain(..excess);
            let _ = self.events.send(TerminalChunk {
                session_id: id.into(),
                offset,
                bytes: bytes.to_vec(),
            });
        }
    }
    pub fn replay(&self, id: &str, after: u64) -> Result<TerminalReplay> {
        let buffers = self.buffers.lock().map_err(|_| Error::Internal)?;
        let buffer = buffers.get(id).ok_or(Error::NotFound)?;
        let first = buffer.end - buffer.bytes.len() as u64;
        let start = after.clamp(first, buffer.end);
        Ok(TerminalReplay {
            start,
            end: buffer.end,
            gap: after < first || after > buffer.end,
            bytes: buffer
                .bytes
                .iter()
                .skip((start - first) as usize)
                .copied()
                .collect(),
        })
    }
}
