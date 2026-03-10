use std::collections::{BTreeMap, BTreeSet, VecDeque};

use nooboard_app::{Transfer, TransferId, TransferState};

const TELEMETRY_WINDOW_MS: i64 = 1_200;
const TELEMETRY_STALE_MS: i64 = 1_500;
const TELEMETRY_MIN_DELTA_MS: i64 = 250;
const TELEMETRY_MAX_SAMPLES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransferTelemetryEstimate {
    pub speed_bps: u64,
    pub eta_seconds: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TransferTelemetrySample {
    observed_at_ms: i64,
    transferred_bytes: u64,
}

#[derive(Debug, Clone, Default)]
pub struct TransferTelemetryCache {
    samples: BTreeMap<TransferId, VecDeque<TransferTelemetrySample>>,
}

impl TransferTelemetryCache {
    pub fn observe_active_transfers(&mut self, active: &[Transfer]) {
        let active_ids = active
            .iter()
            .map(|transfer| transfer.transfer_id.clone())
            .collect::<BTreeSet<_>>();
        self.samples
            .retain(|transfer_id, _| active_ids.contains(transfer_id));

        for transfer in active {
            let sample = TransferTelemetrySample {
                observed_at_ms: transfer.updated_at_ms.max(transfer.started_at_ms),
                transferred_bytes: transfer.transferred_bytes,
            };

            let entry = self
                .samples
                .entry(transfer.transfer_id.clone())
                .or_default();
            if entry.back().copied() == Some(sample) {
                continue;
            }

            entry.push_back(sample);
            prune_samples(entry, sample.observed_at_ms);
        }
    }

    pub fn estimate_for(
        &self,
        transfer: &Transfer,
        now_ms: i64,
    ) -> Option<TransferTelemetryEstimate> {
        if transfer.state != TransferState::InProgress || transfer.file_size == 0 {
            return None;
        }

        let samples = self.samples.get(&transfer.transfer_id)?;
        let oldest = samples.front()?;
        let newest = samples.back()?;

        if now_ms.saturating_sub(newest.observed_at_ms) > TELEMETRY_STALE_MS {
            return None;
        }

        let delta_ms = newest.observed_at_ms.saturating_sub(oldest.observed_at_ms);
        if delta_ms < TELEMETRY_MIN_DELTA_MS {
            return None;
        }

        let delta_bytes = newest
            .transferred_bytes
            .saturating_sub(oldest.transferred_bytes);
        if delta_bytes == 0 {
            return None;
        }

        let speed_bps = delta_bytes
            .saturating_mul(1_000)
            .saturating_div(delta_ms as u64);
        if speed_bps == 0 {
            return None;
        }

        let remaining_bytes = transfer
            .file_size
            .saturating_sub(transfer.transferred_bytes);
        let eta_seconds = if remaining_bytes == 0 {
            Some(0)
        } else {
            Some(remaining_bytes.saturating_add(speed_bps - 1) / speed_bps)
        };

        Some(TransferTelemetryEstimate {
            speed_bps,
            eta_seconds,
        })
    }
}

fn prune_samples(samples: &mut VecDeque<TransferTelemetrySample>, newest_observed_at_ms: i64) {
    while samples.len() > TELEMETRY_MAX_SAMPLES {
        let _ = samples.pop_front();
    }

    while samples.len() > 1 {
        let Some(front) = samples.front() else {
            break;
        };
        if newest_observed_at_ms.saturating_sub(front.observed_at_ms) <= TELEMETRY_WINDOW_MS {
            break;
        }
        let _ = samples.pop_front();
    }
}

#[cfg(test)]
mod tests {
    use nooboard_app::{NoobId, TransferDirection};

    use super::*;

    fn sample_transfer(updated_at_ms: i64, transferred_bytes: u64) -> Transfer {
        Transfer {
            transfer_id: TransferId::new(NoobId::new("peer-a"), 1),
            direction: TransferDirection::Upload,
            peer_noob_id: NoobId::new("peer-a"),
            peer_device_id: "peer-a-device".to_string(),
            file_name: "demo.txt".to_string(),
            file_size: 1_000,
            transferred_bytes,
            state: TransferState::InProgress,
            started_at_ms: 0,
            updated_at_ms,
        }
    }

    #[test]
    fn estimate_uses_recent_window_average() {
        let mut cache = TransferTelemetryCache::default();
        cache.observe_active_transfers(&[sample_transfer(0, 0)]);
        cache.observe_active_transfers(&[sample_transfer(400, 200)]);
        cache.observe_active_transfers(&[sample_transfer(1_000, 800)]);

        let estimate = cache
            .estimate_for(&sample_transfer(1_000, 800), 1_000)
            .expect("estimate");

        assert_eq!(estimate.speed_bps, 800);
        assert_eq!(estimate.eta_seconds, Some(1));
    }

    #[test]
    fn stale_estimate_is_hidden() {
        let mut cache = TransferTelemetryCache::default();
        cache.observe_active_transfers(&[sample_transfer(0, 0)]);
        cache.observe_active_transfers(&[sample_transfer(500, 250)]);

        assert!(
            cache
                .estimate_for(&sample_transfer(500, 250), 2_100)
                .is_none()
        );
    }
}
