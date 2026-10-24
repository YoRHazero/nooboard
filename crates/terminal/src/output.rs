use crate::{TerminalOutputChunk, TerminalOutputError};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
use tokio::sync::watch;

pub(crate) struct OutputBuffer {
    state: Mutex<State>,
    changed: watch::Sender<u64>,
}
struct State {
    bytes: VecDeque<u8>,
    offset: u64,
    capacity: usize,
    closed: bool,
}
pub struct TerminalOutputSubscription {
    pub initial_output: TerminalOutputChunk,
    pub stream: TerminalOutputStream,
}
pub struct TerminalOutputStream {
    buffer: Arc<OutputBuffer>,
    changed: watch::Receiver<u64>,
    cursor: u64,
    done: bool,
}
impl OutputBuffer {
    pub(crate) fn new(capacity: usize) -> Arc<Self> {
        let (changed, _) = watch::channel(0);
        Arc::new(Self {
            state: Mutex::new(State {
                bytes: VecDeque::new(),
                offset: 0,
                capacity,
                closed: false,
            }),
            changed,
        })
    }
    fn trim(state: &mut State) {
        let excess = state.bytes.len().saturating_sub(state.capacity);
        state.bytes.drain(..excess);
        state.offset += excess as u64;
    }
    pub(crate) fn push(&self, bytes: &[u8]) {
        let mut state = self.state.lock().unwrap();
        if state.closed || bytes.is_empty() {
            return;
        }
        // Never allocate beyond the configured replay window for a large incoming chunk.
        if bytes.len() >= state.capacity {
            state.offset += state.bytes.len() as u64 + (bytes.len() - state.capacity) as u64;
            state.bytes.clear();
            let start = bytes.len() - state.capacity;
            state.bytes.extend(&bytes[start..]);
        } else {
            state.bytes.extend(bytes);
            Self::trim(&mut state);
        }
        self.changed.send_modify(|n| *n = n.wrapping_add(1));
    }
    pub(crate) fn resize(&self, capacity: usize) {
        let mut state = self.state.lock().unwrap();
        state.capacity = capacity;
        Self::trim(&mut state);
        self.changed.send_modify(|n| *n = n.wrapping_add(1));
    }
    pub(crate) fn close(&self) {
        let mut state = self.state.lock().unwrap();
        state.closed = true;
        self.changed.send_modify(|n| *n = n.wrapping_add(1));
    }
    pub(crate) fn subscribe(self: &Arc<Self>) -> TerminalOutputSubscription {
        let state = self.state.lock().unwrap();
        let initial_output = TerminalOutputChunk {
            offset: state.offset,
            data: state.bytes.iter().copied().collect(),
        };
        let cursor = state.offset + state.bytes.len() as u64;
        TerminalOutputSubscription {
            initial_output,
            stream: TerminalOutputStream {
                buffer: self.clone(),
                changed: self.changed.subscribe(),
                cursor,
                done: false,
            },
        }
    }
}
impl TerminalOutputStream {
    pub async fn next(
        &mut self,
    ) -> std::result::Result<Option<TerminalOutputChunk>, TerminalOutputError> {
        if self.done {
            return Ok(None);
        }
        loop {
            self.changed.borrow_and_update();
            {
                let state = self.buffer.state.lock().unwrap();
                if self.cursor < state.offset {
                    self.done = true;
                    return Err(TerminalOutputError::Lagged {
                        expected_offset: self.cursor,
                        available_offset: state.offset,
                    });
                }
                let start = (self.cursor - state.offset) as usize;
                if start < state.bytes.len() {
                    let data: Vec<u8> = state.bytes.iter().skip(start).copied().collect();
                    let chunk = TerminalOutputChunk {
                        offset: self.cursor,
                        data,
                    };
                    self.cursor += chunk.data.len() as u64;
                    return Ok(Some(chunk));
                }
                if state.closed {
                    self.done = true;
                    return Ok(None);
                }
            }
            if self.changed.changed().await.is_err() {
                self.done = true;
                return Ok(None);
            }
        }
    }
}

/// Only retains a possible marker prefix, never an unbounded output line.
#[derive(Default)]
pub(crate) struct ReadyFilter {
    prefix: Vec<u8>,
    passthrough: bool,
}
impl ReadyFilter {
    pub(crate) fn feed(&mut self, bytes: &[u8]) -> (Vec<u8>, bool) {
        const LF: &[u8] = b"::nooboard:v1:ready::\n";
        const CRLF: &[u8] = b"::nooboard:v1:ready::\r\n";
        let mut output = Vec::with_capacity(bytes.len());
        let mut ready = false;
        for &byte in bytes {
            if self.passthrough {
                output.push(byte);
                if byte == b'\n' {
                    self.passthrough = false;
                }
            } else {
                self.prefix.push(byte);
                if self.prefix == LF || self.prefix == CRLF {
                    self.prefix.clear();
                    ready = true;
                } else if !LF.starts_with(&self.prefix) && !CRLF.starts_with(&self.prefix) {
                    output.append(&mut self.prefix);
                    self.passthrough = byte != b'\n';
                }
            }
        }
        (output, ready)
    }
    pub(crate) fn finish(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.prefix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ready_marker_is_exact_incremental_and_removed_without_losing_other_bytes() {
        let input = b"hello\r\n::nooboard:v1:ready::\r\n next ::nooboard:v1:ready::\n::nooboard:v1:ready::\nbye";
        for chunk_size in 1..=input.len() {
            let mut filter = ReadyFilter::default();
            let mut output = Vec::new();
            let mut ready = false;
            for chunk in input.chunks(chunk_size) {
                let (bytes, marked) = filter.feed(chunk);
                output.extend(bytes);
                ready |= marked;
            }
            output.extend(filter.finish());
            assert!(ready);
            assert_eq!(output, b"hello\r\n next ::nooboard:v1:ready::\nbye");
        }
        let mut filter = ReadyFilter::default();
        assert!(!filter.feed(b"::nooboard:v1:ready::").1);
        assert_eq!(filter.finish(), b"::nooboard:v1:ready::");
    }
    #[tokio::test]
    async fn replay_handoff_offsets_lag_and_eof_are_explicit() {
        let buffer = OutputBuffer::new(5);
        buffer.push(b"abcdef");
        let mut first = buffer.subscribe();
        assert_eq!(first.initial_output.offset, 1);
        assert_eq!(first.initial_output.data, b"bcdef");
        buffer.push(b"gh");
        let chunk = first.stream.next().await.unwrap().unwrap();
        assert_eq!((chunk.offset, chunk.data), (6, b"gh".to_vec()));
        buffer.push(b"123456");
        assert_eq!(
            first.stream.next().await.unwrap_err(),
            TerminalOutputError::Lagged {
                expected_offset: 8,
                available_offset: 9
            }
        );
        assert!(first.stream.next().await.unwrap().is_none());
        let mut second = buffer.subscribe();
        buffer.push(b"!");
        buffer.close();
        assert_eq!(second.stream.next().await.unwrap().unwrap().data, b"!");
        assert!(second.stream.next().await.unwrap().is_none());
    }
}
