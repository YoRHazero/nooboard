use crate::{Result, TerminalError};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// Runtime tunables. Defaults are data, embedded from config/defaults.json.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalServiceOptions {
    pub stop: TerminalStopOptions,
    pub io: TerminalIoOptions,
    pub output_buffer_bytes: usize,
    pub event_buffer_capacity: usize,
    pub control_queue_capacity: usize,
    pub input_queue_capacity: usize,
    pub transfer_buffer_bytes: usize,
    #[serde(rename = "process_poll_interval_ms", with = "milliseconds")]
    pub process_poll_interval: Duration,
    #[serde(rename = "progress_interval_ms", with = "milliseconds")]
    pub progress_interval: Duration,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalStopOptions {
    #[serde(rename = "stop_grace_timeout_ms", with = "milliseconds")]
    pub stop_grace_timeout: Duration,
    #[serde(rename = "force_stop_timeout_ms", with = "milliseconds")]
    pub force_stop_timeout: Duration,
    #[serde(rename = "shutdown_timeout_ms", with = "milliseconds")]
    pub shutdown_timeout: Duration,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalIoOptions {
    #[serde(rename = "connect_timeout_ms", with = "milliseconds")]
    pub connect_timeout: Duration,
    #[serde(rename = "request_timeout_ms", with = "milliseconds")]
    pub request_timeout: Duration,
}
impl Default for TerminalServiceOptions {
    fn default() -> Self {
        Self::from_json(include_str!("../config/defaults.json"))
            .expect("checked bundled terminal configuration")
    }
}
impl TerminalServiceOptions {
    pub fn from_json(text: &str) -> Result<Self> {
        let options: Self = serde_json::from_str(text)
            .map_err(|_| TerminalError::invalid("invalid terminal options document"))?;
        options.validate()?;
        Ok(options)
    }
    pub(crate) fn validate(&self) -> Result<()> {
        for duration in [
            self.stop.stop_grace_timeout,
            self.stop.force_stop_timeout,
            self.stop.shutdown_timeout,
            self.io.connect_timeout,
            self.io.request_timeout,
            self.process_poll_interval,
            self.progress_interval,
        ] {
            if duration.as_millis() == 0 || Instant::now().checked_add(duration).is_none() {
                return Err(TerminalError::invalid(
                    "timeout must be positive and representable",
                ));
            }
        }
        for capacity in [
            self.output_buffer_bytes,
            self.event_buffer_capacity,
            self.control_queue_capacity,
            self.input_queue_capacity,
            self.transfer_buffer_bytes,
        ] {
            if capacity == 0 || capacity > isize::MAX as usize {
                return Err(TerminalError::invalid(
                    "buffer capacity must be positive and representable",
                ));
            }
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
