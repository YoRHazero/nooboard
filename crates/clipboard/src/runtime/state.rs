use crate::{Error, Origin, Result, Snapshot, backend::Observation};

#[derive(Default)]
pub(super) struct State {
    pub native_revision: Option<u64>,
    pub snapshot: Option<Snapshot>,
    last_write: Option<u64>,
    sequence: u64,
}
impl State {
    pub fn observe(&mut self, observed: Observation, written: bool) -> Result<Snapshot> {
        if written {
            self.last_write = Some(observed.revision);
        } else if self.last_write != Some(observed.revision) {
            self.last_write = None;
        }
        let origin = if self.last_write == Some(observed.revision)
            || observed.origin == Origin::Application
        {
            Origin::Application
        } else {
            Origin::External
        };
        if !written
            && self.native_revision == Some(observed.revision)
            && let Some(previous) = &self.snapshot
            && previous.content == observed.content
        {
            return Ok(previous.clone());
        }
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| Error::backend("observation sequence", "exhausted"))?;
        self.native_revision = Some(observed.revision);
        let snapshot = Snapshot {
            revision: self.sequence,
            content: observed.content,
            origin,
        };
        self.snapshot = Some(snapshot.clone());
        Ok(snapshot)
    }
}
