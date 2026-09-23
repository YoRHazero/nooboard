//! mDNS hints only. Authentication always comes from pairing or the pinned TLS certificate.
use super::NearbyDevice as Device;
use mdns_sd::{ScopedIp, ServiceDaemon, ServiceEvent, ServiceInfo};
use std::{
    collections::BTreeMap,
    net::{SocketAddr, SocketAddrV6},
    time::{Duration, Instant},
};
use tokio::{sync::watch, task::JoinHandle};

pub const SERVICE_TYPE: &str = "_nooboard._tcp.local.";
pub struct Discovery {
    daemon: ServiceDaemon,
    own_id: String,
    registration: Option<String>,
    pump: Option<JoinHandle<()>>,
    devices: watch::Sender<Vec<Device>>,
    refreshed: Option<Instant>,
}
impl Discovery {
    pub fn new(own_id: String) -> Result<Self, String> {
        Self::with_daemon(own_id, ServiceDaemon::new().map_err(|e| e.to_string())?)
    }
    fn with_daemon(own_id: String, daemon: ServiceDaemon) -> Result<Self, String> {
        let (devices, _) = watch::channel(Vec::new());
        let mut value = Self {
            daemon,
            own_id,
            registration: None,
            pump: None,
            devices,
            refreshed: None,
        };
        value
            .daemon
            .set_ip_check_interval(5)
            .map_err(|e| e.to_string())?;
        value.refresh()?;
        Ok(value)
    }
    pub async fn shutdown(mut self) -> Result<(), String> {
        if let Some(task) = self.pump.take() {
            task.abort();
            let _ = task.await;
        }
        if let Some(name) = self.registration.take() {
            let _ = self.daemon.unregister(&name);
        }
        let finished = self.daemon.shutdown().map_err(|e| e.to_string())?;
        tokio::time::timeout(Duration::from_secs(3), finished.recv_async())
            .await
            .map_err(|_| "discovery shutdown timed out".to_owned())?
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn subscribe(&self) -> watch::Receiver<Vec<Device>> {
        self.devices.subscribe()
    }
    pub fn advertise(
        &mut self,
        name: &str,
        pairing: SocketAddr,
        sync_port: u16,
        enabled: bool,
    ) -> Result<(), String> {
        if !enabled {
            if let Some(fullname) = self.registration.take() {
                self.daemon
                    .unregister(&fullname)
                    .map_err(|e| e.to_string())?;
            }
            return Ok(());
        }
        // Each DNS-SD TXT item is limited to 255 bytes, including its key.
        let mut split = name.len().min(240);
        while !name.is_char_boundary(split) {
            split -= 1;
        }
        let (head, tail) = name.split_at(split);
        let properties = [
            ("id", self.own_id.as_str()),
            ("name", head),
            ("name2", tail),
            ("version", "1"),
            ("sync", &sync_port.to_string()),
        ];
        let hostname = format!("nooboard-{}.local.", &self.own_id[..16]);
        let mut service = ServiceInfo::new(
            SERVICE_TYPE,
            &self.own_id[..24],
            &hostname,
            if pairing.ip().is_unspecified() {
                String::new()
            } else {
                pairing.ip().to_string()
            },
            pairing.port(),
            &properties[..],
        )
        .map_err(|e| e.to_string())?;
        if pairing.ip().is_unspecified() {
            service = service.enable_addr_auto();
        }
        self.registration = Some(service.get_fullname().to_owned());
        self.daemon.register(service).map_err(|e| e.to_string())
    }
    pub fn refresh(&mut self) -> Result<(), String> {
        if self
            .refreshed
            .is_some_and(|at| at.elapsed() < Duration::from_secs(3))
        {
            return Ok(());
        }
        if let Some(task) = self.pump.take() {
            task.abort();
            self.daemon
                .stop_browse(SERVICE_TYPE)
                .map_err(|e| e.to_string())?;
        }
        let receiver = self
            .daemon
            .browse(SERVICE_TYPE)
            .map_err(|e| e.to_string())?;
        let devices = self.devices.clone();
        let own_id = self.own_id.clone();
        self.pump = Some(tokio::spawn(async move {
            // Rebuild from the daemon's live cache on every browse, so refresh cannot
            // retain entries whose removal arrived while replacing the subscription.
            let mut known: BTreeMap<String, Device> = BTreeMap::new();
            devices.send_replace(Vec::new());
            while let Ok(event) = receiver.recv_async().await {
                match event {
                    ServiceEvent::ServiceResolved(info) => {
                        let Some(id) = info.get_property_val_str("id").filter(|id| {
                            id.len() == 64
                                && id.bytes().all(|b| b.is_ascii_hexdigit())
                                && *id != own_id
                        }) else {
                            continue;
                        };
                        let name = format!(
                            "{}{}",
                            info.get_property_val_str("name").unwrap_or_default(),
                            info.get_property_val_str("name2").unwrap_or_default()
                        );
                        if !crate::identity::valid_device_name(&name) {
                            continue;
                        }
                        if info.get_property_val_str("version") != Some("1") {
                            continue;
                        }
                        let Some(sync_port) = info
                            .get_property_val_str("sync")
                            .and_then(|p| p.parse::<u16>().ok())
                            .filter(|p| *p > 0)
                        else {
                            continue;
                        };
                        let mut addresses = info
                            .get_addresses()
                            .iter()
                            .filter_map(|ip| match ip {
                                ScopedIp::V4(v4)
                                    if !v4.addr().is_unspecified() && !v4.addr().is_multicast() =>
                                {
                                    Some(
                                        SocketAddr::new((*v4.addr()).into(), info.get_port())
                                            .to_string(),
                                    )
                                }
                                ScopedIp::V6(v6)
                                    if !v6.addr().is_unspecified() && !v6.addr().is_multicast() =>
                                {
                                    Some(
                                        SocketAddrV6::new(
                                            *v6.addr(),
                                            info.get_port(),
                                            0,
                                            v6.scope_id().index,
                                        )
                                        .to_string(),
                                    )
                                }
                                _ => None,
                            })
                            .collect::<Vec<_>>();
                        addresses.sort_by_key(|a| {
                            (
                                a.parse::<SocketAddr>().is_ok_and(|a| a.ip().is_loopback()),
                                a.starts_with('['),
                                a.clone(),
                            )
                        });
                        addresses.dedup();
                        addresses.truncate(16);
                        if addresses.is_empty()
                            || info.get_port() == 0
                            || (known.len() >= 256 && !known.contains_key(info.get_fullname()))
                        {
                            continue;
                        }
                        known.insert(
                            info.get_fullname().to_owned(),
                            Device {
                                key: info.get_fullname().into(),
                                noob_id: id.into(),
                                device_name: name,
                                addresses,
                                sync_port,
                            },
                        );
                    }
                    ServiceEvent::ServiceRemoved(_, fullname) => {
                        known.remove(&fullname);
                    }
                    _ => continue,
                }
                let next = known.values().cloned().collect::<Vec<_>>();
                devices.send_if_modified(|old| {
                    if *old == next {
                        false
                    } else {
                        *old = next;
                        true
                    }
                });
            }
        }));
        self.refreshed = Some(Instant::now());
        Ok(())
    }
}
impl Drop for Discovery {
    fn drop(&mut self) {
        if let Some(task) = self.pump.take() {
            task.abort();
        }
        if let Some(name) = &self.registration {
            let _ = self.daemon.unregister(name);
        }
        let _ = self.daemon.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore = "requires multicast loopback sockets"]
    async fn mdns_discovers_updates_and_withdraws_a_peer_without_trusting_it() {
        let port = std::net::UdpSocket::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let make = |id: String| {
            let daemon = ServiceDaemon::new_with_port(port).unwrap();
            daemon.disable_interface(mdns_sd::IfKind::All).unwrap();
            daemon
                .enable_interface(mdns_sd::IfKind::LoopbackV4)
                .unwrap();
            Discovery::with_daemon(id, daemon).unwrap()
        };
        let a = make("a".repeat(64));
        let mut b = make("b".repeat(64));
        let mut seen = a.subscribe();
        let address = "127.0.0.1:29117".parse().unwrap();
        b.advertise(&"🐦".repeat(80), address, 29116, true).unwrap();
        tokio::time::timeout(Duration::from_secs(15), async {
            while seen.borrow().is_empty() {
                seen.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
        assert_eq!(seen.borrow()[0].addresses, vec![address.to_string()]);
        assert_eq!(seen.borrow()[0].device_name, "🐦".repeat(80));
        b.advertise("改名后的肥啾", address, 29118, true).unwrap();
        tokio::time::timeout(Duration::from_secs(15), async {
            while seen.borrow()[0].sync_port != 29118 {
                seen.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
        assert_eq!(seen.borrow()[0].device_name, "改名后的肥啾");
        b.advertise("", address, 29118, false).unwrap();
        tokio::time::timeout(Duration::from_secs(15), async {
            while !seen.borrow().is_empty() {
                seen.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
    }
}
