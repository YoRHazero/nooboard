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
    line: ReadyLineState,
}
#[derive(Default)]
enum ReadyLineState {
    #[default]
    Matching,
    Passthrough,
    Discard,
}
impl ReadyFilter {
    pub(crate) fn feed(&mut self, bytes: &[u8]) -> (Vec<u8>, bool) {
        const MARKER: &[u8] = b"::nooboard:v1:ready::";
        let mut output = Vec::with_capacity(bytes.len());
        let mut ready = false;
        for &byte in bytes {
            match self.line {
                ReadyLineState::Discard => {
                    if byte == b'\n' {
                        self.line = ReadyLineState::Matching;
                    }
                }
                ReadyLineState::Passthrough => {
                    output.push(byte);
                    if byte == b'\n' {
                        self.line = ReadyLineState::Matching;
                    }
                }
                ReadyLineState::Matching => {
                    self.prefix.push(byte);
                    if self.prefix == MARKER {
                        self.prefix.clear();
                        ready = true;
                        // Signal immediately, then hide the rest of this protocol line.
                        // Nested PTYs can turn its newline into CRCRLF.
                        self.line = ReadyLineState::Discard;
                    } else if !MARKER.starts_with(&self.prefix) {
                        output.append(&mut self.prefix);
                        if byte != b'\n' {
                            self.line = ReadyLineState::Passthrough;
                        }
                    }
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

    fn assert_filtered(input: &[u8], expected: &[u8], expected_ready: bool) {
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
            assert_eq!(ready, expected_ready, "chunk size {chunk_size}");
            assert_eq!(output, expected, "chunk size {chunk_size}");
        }
    }

    #[test]
    fn ready_marker_lines_are_removed_across_chunks_and_line_endings() {
        let input = b"hello\r\n::nooboard:v1:ready::\r\n next ::nooboard:v1:ready::\n::nooboard:v1:ready::\n::nooboard:v1:ready::\r\r\n::nooboard:v1:ready:: trailing text\r\nbye";
        assert_filtered(input, b"hello\r\n next ::nooboard:v1:ready::\nbye", true);
    }

    #[test]
    fn ready_marker_signals_before_any_suffix_or_newline_arrives() {
        let mut filter = ReadyFilter::default();
        assert_eq!(filter.feed(b"::nooboard:v1:rea"), (vec![], false));
        assert_eq!(filter.feed(b"dy::"), (vec![], true));
        assert_eq!(filter.feed(b" trailing text\r\r"), (vec![], false));
        assert_eq!(filter.feed(b"\nnext\n"), (b"next\n".to_vec(), false));
        assert!(filter.finish().is_empty());
        assert_filtered(b"::nooboard:v1:ready::", b"", true);
    }

    #[test]
    fn ready_marker_requires_the_complete_prefix_at_the_start_of_a_line() {
        for input in [
            b"prompt: ::nooboard:v1:ready::\n".as_slice(),
            b" ::nooboard:v1:ready::\n",
            b"\x1b[32m::nooboard:v1:ready::\n",
            b"prompt\r::nooboard:v1:ready::\n",
            b"::nooboard:v1:ready:\n",
            b"::nooboard:v1:ready!::\n",
        ] {
            assert_filtered(input, input, false);
        }
        let marker = b"::nooboard:v1:ready::";
        for length in 1..marker.len() {
            assert_filtered(&marker[..length], &marker[..length], false);
        }
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
