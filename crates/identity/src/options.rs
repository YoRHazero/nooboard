use crate::{Error, IdentityConfig, Result, SecretContext};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityOptions {
    pub queue_capacity: usize,
    #[serde(rename = "signing_timeout_ms", with = "milliseconds")]
    pub signing_timeout: Duration,
    #[serde(rename = "shutdown_timeout_ms", with = "milliseconds")]
    pub shutdown_timeout: Duration,
    #[serde(rename = "credential_lock_timeout_ms", with = "milliseconds")]
    pub credential_lock_timeout: Duration,
    #[serde(rename = "credential_lock_poll_interval_ms", with = "milliseconds")]
    pub credential_lock_poll_interval: Duration,
    pub max_secret_bytes: usize,
    pub max_signing_bytes: usize,
    pub max_context_bytes: usize,
}
impl Default for IdentityOptions {
    fn default() -> Self {
        Self::from_json(include_str!("../config/defaults.json")).expect("bundled identity options")
    }
}
impl IdentityOptions {
    pub fn from_json(text: &str) -> Result<Self> {
        let options: Self = serde_json::from_str(text).map_err(|_| Error::InvalidInput)?;
        options.validate()?;
        Ok(options)
    }
    pub(crate) fn validate(&self) -> Result<()> {
        if self.queue_capacity > tokio::sync::Semaphore::MAX_PERMITS {
            return Err(Error::InvalidInput);
        }
        for duration in [
            self.signing_timeout,
            self.shutdown_timeout,
            self.credential_lock_timeout,
            self.credential_lock_poll_interval,
        ] {
            if duration.as_millis() == 0 || Instant::now().checked_add(duration).is_none() {
                return Err(Error::InvalidInput);
            }
        }
        for capacity in [
            self.queue_capacity,
            self.max_secret_bytes,
            self.max_signing_bytes,
            self.max_context_bytes,
        ] {
            if capacity == 0 || capacity > (isize::MAX as usize).saturating_sub(16) {
                return Err(Error::InvalidInput);
            }
        }
        if self.credential_lock_poll_interval > self.credential_lock_timeout {
            return Err(Error::InvalidInput);
        }
        Ok(())
    }
    pub(crate) fn validate_context(&self, context: &SecretContext) -> Result<()> {
        if context.purpose.trim().is_empty()
            || context.purpose.chars().any(char::is_control)
            || context
                .purpose
                .len()
                .saturating_add(context.associated_data.len())
                > self.max_context_bytes
        {
            return Err(Error::InvalidInput);
        }
        Ok(())
    }
}
impl IdentityConfig {
    pub(crate) fn validate(&self) -> Result<()> {
        if let Self::System { profile } = self
            && (profile.is_empty() || profile.len() > 256 || profile.chars().any(char::is_control))
        {
            return Err(Error::InvalidInput);
        }
        Ok(())
    }
}
mod milliseconds {
    use super::*;
    use serde::{Deserializer, Serializer};
    pub fn serialize<S: Serializer>(
        value: &Duration,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        u64::try_from(value.as_millis())
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Duration, D::Error> {
        Ok(Duration::from_millis(u64::deserialize(deserializer)?))
    }
}
