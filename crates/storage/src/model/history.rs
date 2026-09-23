use super::{MAX_TEXT_BYTES, text};
use crate::{Error, Result};

/// Store-scoped identity; callers must not infer ordering from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HistoryId(i64);
impl HistoryId {
    pub fn value(self) -> i64 {
        self.0
    }
}
impl TryFrom<i64> for HistoryId {
    type Error = Error;
    fn try_from(value: i64) -> Result<Self> {
        if value > 0 {
            Ok(Self(value))
        } else {
            Err(Error::invalid("history ID"))
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HistorySource {
    Local,
    Remote(String),
}
impl HistorySource {
    pub(crate) fn validate(&self) -> Result<()> {
        if let Self::Remote(id) = self {
            if id.is_empty() {
                return Err(Error::invalid("history source"));
            }
            text(id, 1024, "history source")?;
        }
        Ok(())
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct NewHistoryEntry {
    pub text: String,
    pub source: HistorySource,
    /// Unix milliseconds, supplied by the caller.
    pub copied_at_ms: i64,
}
impl NewHistoryEntry {
    pub(crate) fn validate(&self) -> Result<()> {
        text(&self.text, MAX_TEXT_BYTES, "history text")?;
        self.source.validate()?;
        if self.copied_at_ms < 0 {
            return Err(Error::invalid("history timestamp"));
        }
        Ok(())
    }
}
impl std::fmt::Debug for NewHistoryEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NewHistoryEntry")
            .field("bytes", &self.text.len())
            .field("source", &self.source)
            .field("copied_at_ms", &self.copied_at_ms)
            .finish()
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryEntry {
    pub id: HistoryId,
    pub entry: NewHistoryEntry,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Retention {
    /// Zero is permitted for pruning, but not for recording.
    pub max_entries: u32,
    /// Entries strictly older than this timestamp are removed.
    pub oldest_ms: i64,
}
#[derive(Clone, Debug)]
pub struct RecordHistory {
    pub entry: NewHistoryEntry,
    pub retention: Retention,
}
impl RecordHistory {
    pub(crate) fn validate(&self) -> Result<()> {
        self.entry.validate()?;
        if self.retention.max_entries == 0 || self.retention.oldest_ms > self.entry.copied_at_ms {
            return Err(Error::invalid("history retention"));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordOutcome {
    pub id: HistoryId,
    pub inserted: bool,
    /// A backdated entry may immediately fall outside the retained count.
    pub retained: bool,
    pub pruned: u64,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SourceFilter {
    #[default]
    All,
    Local,
    Remote,
    Device(String),
}
#[derive(Clone, Debug)]
pub struct HistoryQuery {
    /// Literal, case-sensitive substring; no wildcard interpretation or normalization.
    pub contains: String,
    pub source: SourceFilter,
    pub limit: u32,
    pub offset: u64,
}
impl Default for HistoryQuery {
    fn default() -> Self {
        Self {
            contains: String::new(),
            source: SourceFilter::All,
            limit: 100,
            offset: 0,
        }
    }
}
impl HistoryQuery {
    pub(crate) fn validate(&self) -> Result<()> {
        text(&self.contains, MAX_TEXT_BYTES, "history search")?;
        if !(1..=1000).contains(&self.limit) || self.offset > (i64::MAX as u64 - 1001) {
            return Err(Error::invalid("history pagination"));
        }
        if let SourceFilter::Device(id) = &self.source {
            HistorySource::Remote(id.clone()).validate()?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryPage {
    pub entries: Vec<HistoryEntry>,
    /// Offset pagination observes a fresh snapshot on each request.
    pub next_offset: Option<u64>,
}
