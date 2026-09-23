use crate::{error::ErrorKind, identity::PublicIdentity};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PairingId(pub(crate) String);
impl PairingId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Clone)]
pub struct PairingStatus {
    pub id: PairingId,
    pub peer: Option<PublicIdentity>,
    pub incoming: bool,
    pub stage: Stage,
    pub code: Option<String>,
    pub attempts_left: u8,
    pub error: Option<ErrorKind>,
}
impl std::fmt::Debug for PairingStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PairingStatus")
            .field("id", &self.id)
            .field("stage", &self.stage)
            .field("incoming", &self.incoming)
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub enum Stage {
    Requesting,
    AwaitingApproval,
    ShowingCode,
    EnteringCode,
    Verifying,
    Saving,
    Completed,
    Failed,
}
