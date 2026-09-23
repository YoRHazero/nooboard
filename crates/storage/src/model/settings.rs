use super::{MAX_TEXT_BYTES, text};
use crate::{Error, Result};
use std::collections::HashSet;

#[derive(Clone, PartialEq, Eq)]
pub struct Setting {
    pub key: String,
    /// Opaque UTF-8 application document; storage does not interpret its contents.
    pub value: String,
}
impl std::fmt::Debug for Setting {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Setting")
            .field("key", &self.key)
            .field("bytes", &self.value.len())
            .finish()
    }
}
#[derive(Clone, Debug, Default)]
pub struct SettingsChanges {
    pub put: Vec<Setting>,
    pub delete: Vec<String>,
}
pub(crate) fn validate_key(key: &str) -> Result<()> {
    if key.is_empty() {
        return Err(Error::invalid("setting key"));
    }
    text(key, 256, "setting key")
}
impl SettingsChanges {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.put.len().saturating_add(self.delete.len()) > 1024 {
            return Err(Error::invalid("settings batch size"));
        }
        let mut keys = HashSet::new();
        for setting in &self.put {
            validate_key(&setting.key)?;
            text(&setting.value, MAX_TEXT_BYTES, "setting value")?;
            if !keys.insert(setting.key.as_str()) {
                return Err(Error::invalid("duplicate setting key"));
            }
        }
        for key in &self.delete {
            validate_key(key)?;
            if !keys.insert(key.as_str()) {
                return Err(Error::invalid("duplicate setting key"));
            }
        }
        Ok(())
    }
}
