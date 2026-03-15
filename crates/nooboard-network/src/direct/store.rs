use std::net::{Ipv4Addr, SocketAddr};

use crate::config::DirectSeedConfig;
use crate::errors::{NetworkError, NetworkResult};
use crate::{DirectSeedId, DirectSeedInfo, UpsertDirectSeedInput};

#[derive(Debug, Clone, Default)]
pub(crate) struct DirectSeedStore {
    seeds: Vec<DirectSeedInfo>,
}

impl DirectSeedStore {
    pub(crate) fn from_configs(configs: &[DirectSeedConfig]) -> Self {
        Self {
            seeds: configs
                .iter()
                .map(DirectSeedInfo::from_seed_config)
                .collect(),
        }
    }

    pub(crate) fn list(&self) -> Vec<DirectSeedInfo> {
        self.seeds.clone()
    }

    pub(crate) fn upsert(&mut self, input: UpsertDirectSeedInput) -> NetworkResult<DirectSeedId> {
        validate_upsert_input(&input)?;

        let id = input.id.unwrap_or_else(DirectSeedId::new);
        let next = DirectSeedInfo {
            id,
            label: input.label,
            host: input.host,
            port: input.port,
            enabled: input.enabled,
            learned_device_id: None,
            last_connected_addr: None,
        };

        if let Some(index) = self.seeds.iter().position(|seed| seed.id == id) {
            let learned_device_id = self.seeds[index].learned_device_id.clone();
            let last_connected_addr = self.seeds[index].last_connected_addr;
            self.seeds[index] = DirectSeedInfo {
                learned_device_id,
                last_connected_addr,
                ..next
            };
        } else {
            self.seeds.push(next);
        }

        Ok(id)
    }

    pub(crate) fn remove(&mut self, id: DirectSeedId) -> bool {
        let before = self.seeds.len();
        self.seeds.retain(|seed| seed.id != id);
        before != self.seeds.len()
    }

    pub(crate) fn search(&self, query: &str) -> Vec<DirectSeedInfo> {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return self.list();
        }

        self.seeds
            .iter()
            .filter(|seed| seed.matches_query(&query))
            .cloned()
            .collect()
    }

    pub(crate) fn get(&self, id: DirectSeedId) -> Option<&DirectSeedInfo> {
        self.seeds.iter().find(|seed| seed.id == id)
    }

    pub(crate) fn get_cloned(&self, id: DirectSeedId) -> Option<DirectSeedInfo> {
        self.get(id).cloned()
    }

    pub(crate) fn resolve_ipv4_addr(&self, id: DirectSeedId) -> Option<SocketAddr> {
        self.get(id).and_then(DirectSeedInfo::parsed_socket_addr)
    }

    pub(crate) fn record_success(
        &mut self,
        id: DirectSeedId,
        device_id: &str,
        remote_addr: SocketAddr,
    ) {
        if let Some(seed) = self.seeds.iter_mut().find(|seed| seed.id == id) {
            seed.learned_device_id = Some(device_id.to_string());
            seed.last_connected_addr = Some(remote_addr);
        }
    }
}

impl DirectSeedInfo {
    pub(crate) fn from_seed_config(seed: &DirectSeedConfig) -> Self {
        Self {
            id: seed.public_id(),
            label: seed.label.clone(),
            host: seed.host.clone(),
            port: seed.port,
            enabled: seed.enabled,
            learned_device_id: None,
            last_connected_addr: None,
        }
    }

    pub(crate) fn matches_query(&self, query: &str) -> bool {
        self.label.to_lowercase().contains(query)
            || self.host.to_lowercase().contains(query)
            || self
                .learned_device_id
                .as_ref()
                .is_some_and(|device_id| device_id.to_lowercase().contains(query))
    }

    pub(crate) fn parsed_socket_addr(&self) -> Option<SocketAddr> {
        let host = self.host.parse::<Ipv4Addr>().ok()?;
        Some(SocketAddr::new(host.into(), self.port))
    }
}

fn validate_upsert_input(input: &UpsertDirectSeedInput) -> NetworkResult<()> {
    if input.host.trim().is_empty() {
        return Err(NetworkError::InvalidConfig(
            "direct seed host must not be empty".to_string(),
        ));
    }
    if input.port == 0 {
        return Err(NetworkError::InvalidConfig(
            "direct seed port must be > 0".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_preserves_existing_position() {
        let mut store = DirectSeedStore::default();

        let alpha = store
            .upsert(UpsertDirectSeedInput {
                id: None,
                label: "alpha".to_string(),
                host: "10.0.0.1".to_string(),
                port: 1,
                enabled: true,
            })
            .expect("insert alpha");
        let beta = store
            .upsert(UpsertDirectSeedInput {
                id: None,
                label: "beta".to_string(),
                host: "10.0.0.2".to_string(),
                port: 2,
                enabled: true,
            })
            .expect("insert beta");

        store
            .upsert(UpsertDirectSeedInput {
                id: Some(alpha),
                label: "alpha-updated".to_string(),
                host: "10.0.0.3".to_string(),
                port: 3,
                enabled: true,
            })
            .expect("update alpha");

        let seeds = store.list();
        assert_eq!(seeds.len(), 2);
        assert_eq!(seeds[0].id, alpha);
        assert_eq!(seeds[0].label, "alpha-updated");
        assert_eq!(seeds[1].id, beta);
    }

    #[test]
    fn search_matches_label_host_and_device_id() {
        let mut store = DirectSeedStore::default();
        let seed_id = store
            .upsert(UpsertDirectSeedInput {
                id: None,
                label: "Home Desktop".to_string(),
                host: "desktop.local".to_string(),
                port: 17890,
                enabled: true,
            })
            .expect("seed");

        let seed = store
            .seeds
            .iter_mut()
            .find(|seed| seed.id == seed_id)
            .expect("seed exists");
        seed.learned_device_id = Some("home-device".to_string());

        assert_eq!(store.search("desktop").len(), 1);
        assert_eq!(store.search("local").len(), 1);
        assert_eq!(store.search("device").len(), 1);
    }
}
