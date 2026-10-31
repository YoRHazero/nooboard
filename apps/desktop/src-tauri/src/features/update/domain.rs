/// Only a verified, downloaded update is exposed to the UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadyUpdate {
    pub version: String,
    pub installing: bool,
}
