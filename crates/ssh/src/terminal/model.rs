use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize)]
pub struct PtySize {
    pub columns: u32,
    pub rows: u32,
}
impl Default for PtySize {
    fn default() -> Self {
        Self {
            columns: 80,
            rows: 24,
        }
    }
}
impl PtySize {
    pub(crate) fn validate(self) -> crate::Result<()> {
        if self.columns == 0 || self.rows == 0 || self.columns > 4096 || self.rows > 4096 {
            Err(crate::Error::Limit)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Debug)]
pub struct TerminalChunk {
    pub session_id: String,
    pub offset: u64,
    pub bytes: Vec<u8>,
}
#[derive(Clone, Debug)]
pub struct TerminalReplay {
    pub start: u64,
    pub end: u64,
    /// True means output preceding `start` was evicted. Never silently splice it.
    pub gap: bool,
    pub bytes: Vec<u8>,
}
