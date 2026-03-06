use std::collections::HashSet;
use std::fmt::{Display, Formatter};

use uuid::Uuid;

use crate::AppError;

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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NoobId(String);

impl NoobId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Targets {
    #[default]
    All,
    Nodes(Vec<NoobId>),
}

impl Targets {
    pub fn all() -> Self {
        Self::All
    }

    pub fn nodes(nodes: Vec<NoobId>) -> Self {
        Self::Nodes(nodes)
    }

    pub(crate) fn should_send(&self) -> bool {
        match self {
            Self::All => true,
            Self::Nodes(nodes) => nodes.iter().any(|node| !node.as_str().trim().is_empty()),
        }
    }

    pub(crate) fn to_sync_targets(&self) -> Option<Vec<String>> {
        match self {
            Self::All => None,
            Self::Nodes(nodes) => {
                let mut seen = HashSet::new();
                let normalized: Vec<String> = nodes
                    .iter()
                    .map(|node| node.as_str().trim().to_string())
                    .filter(|node| !node.is_empty())
                    .filter(|node| seen.insert(node.clone()))
                    .collect();
                Some(normalized)
            }
        }
    }
}
