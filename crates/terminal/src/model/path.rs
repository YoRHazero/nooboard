use super::{Result, TerminalError, TerminalErrorKind};
use serde::{Deserialize, Serialize};
use std::fmt;

/// An absolute path in the server's namespace, never a local PathBuf.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RemotePath(String);
impl RemotePath {
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        let bytes = value.as_bytes();
        let absolute = value.starts_with('/')
            || (bytes.len() >= 3
                && bytes[0].is_ascii_alphabetic()
                && bytes[1] == b':'
                && bytes[2] == b'/');
        if !absolute || value.contains('\0') {
            return Err(TerminalError::new(
                TerminalErrorKind::InvalidRequest,
                "remote path must be absolute and contain no NUL",
            ));
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub(crate) fn join(&self, name: &str) -> Result<Self> {
        if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\0']) {
            return Err(TerminalError::invalid("invalid remote entry name"));
        }
        Self::new(format!("{}/{}", self.0.trim_end_matches('/'), name))
    }
    pub(crate) fn temporary_sibling(&self) -> Result<Self> {
        let (parent, name) = self
            .0
            .rsplit_once('/')
            .ok_or_else(|| TerminalError::invalid("missing parent directory"))?;
        if name.is_empty() {
            return Err(TerminalError::invalid("file path has no name"));
        }
        Self::new(format!("{parent}/.nooboard-{}.tmp", uuid::Uuid::new_v4()))
    }
}
impl TryFrom<String> for RemotePath {
    type Error = TerminalError;
    fn try_from(s: String) -> Result<Self> {
        Self::new(s)
    }
}
impl From<RemotePath> for String {
    fn from(p: RemotePath) -> Self {
        p.0
    }
}
impl fmt::Display for RemotePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
