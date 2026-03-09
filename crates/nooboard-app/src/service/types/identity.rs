use std::fmt::{Display, Formatter};

use uuid::Uuid;

use crate::AppError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalIdentity {
    pub noob_id: NoobId,
    pub device_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EventId(Uuid);

impl EventId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Display for EventId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<Uuid> for EventId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl TryFrom<&str> for EventId {
    type Error = AppError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Uuid::parse_str(value)
            .map(Self)
            .map_err(|_| AppError::InvalidEventId {
                event_id: value.to_string(),
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NoobId(String);

impl NoobId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Display for NoobId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TransferId {
    peer_noob_id: NoobId,
    raw_id: u32,
}

impl TransferId {
    pub fn new(peer_noob_id: NoobId, raw_id: u32) -> Self {
        Self {
            peer_noob_id,
            raw_id,
        }
    }

    pub fn peer_noob_id(&self) -> &NoobId {
        &self.peer_noob_id
    }

    pub fn raw_id(&self) -> u32 {
        self.raw_id
    }
}

impl Display for TransferId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.peer_noob_id, self.raw_id)
    }
}
