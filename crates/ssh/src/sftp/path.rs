use crate::{Error, Result};
use serde::{Deserialize, Serialize};

/// A path in the SFTP server's namespace, never a path on the client machine.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct RemotePath(String);
impl RemotePath {
    pub fn new(value: String) -> Result<Self> {
        if value.is_empty() || value.len() > 32768 || value.contains('\0') {
            return Err(Error::InvalidPath);
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn resolve(&self, input: &str) -> Result<Self> {
        // Only drive-absolute Windows syntax is translated. A backslash in a
        // POSIX filename is data. The server remains the final path authority.
        let drive_absolute = input.as_bytes().get(1) == Some(&b':')
            && input
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphabetic)
            && matches!(input.as_bytes().get(2), Some(b'/' | b'\\'));
        if drive_absolute {
            Self::new(input.replace('\\', "/"))
        } else if input.starts_with('/') {
            Self::new(input.into())
        } else {
            Self::new(format!("{}/{}", self.0.trim_end_matches('/'), input))
        }
    }
    pub(crate) fn child(&self, name: &str) -> Result<Self> {
        if name.is_empty()
            || name == "."
            || name == ".."
            || name.contains('/')
            || name.contains('\0')
        {
            return Err(Error::InvalidPath);
        }
        // A directory entry is one literal component, never a drive-absolute
        // navigation expression. POSIX backslashes remain legal filename data.
        Self::new(format!("{}/{}", self.0.trim_end_matches('/'), name))
    }
}
impl<'de> Deserialize<'de> for RemotePath {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

pub(crate) async fn validate(
    client: &super::client::Client,
    base: &RemotePath,
    input: &str,
) -> Result<RemotePath> {
    let candidate = base.resolve(input)?;
    let response = client.raw.realpath(candidate.as_str()).await?;
    if response.files.len() != 1 {
        return Err(Error::InvalidPath);
    }
    let canonical = RemotePath::new(response.files[0].filename.clone())?;
    let attrs = client.raw.stat(canonical.as_str()).await?.attrs;
    if attrs.permissions.map(|v| v & 0o170000) != Some(0o040000) {
        return Err(Error::NotDirectory);
    }
    Ok(canonical)
}
