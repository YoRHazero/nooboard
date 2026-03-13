use std::net::{IpAddr, SocketAddr};

use mdns_sd::{IfKind, ServiceDaemon, ServiceEvent, ServiceInfo};
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;

use crate::errors::DiscoveryError;
use crate::lan::peer_index::LanServiceRecord;

pub(crate) const SERVICE_TYPE: &str = "_nooboard._tcp.local.";
const TXT_PROTOCOL_VERSION: &str = "pv";
const TXT_NOOB_ID: &str = "nid";
const TXT_DEVICE_ID: &str = "did";
const TXT_BOOT_ID: &str = "bid";

#[derive(Debug, Clone)]
pub(crate) struct LanRuntimeConfig {
    pub(crate) local_noob_id: String,
    pub(crate) local_device_id: String,
    pub(crate) boot_id: String,
    pub(crate) listen_port: u16,
    pub(crate) advertise_addrs: Vec<std::net::Ipv4Addr>,
}

#[derive(Debug)]
pub(crate) enum LanRuntimeEvent {
    Resolved(LanServiceRecord),
    Removed(String),
}

pub(crate) struct LanTaskHandle {
    shutdown_tx: broadcast::Sender<()>,
    task: JoinHandle<()>,
}

impl LanTaskHandle {
    pub(crate) async fn shutdown(self) {
        let _ = self.shutdown_tx.send(());
        let mut task = self.task;
        if tokio::time::timeout(std::time::Duration::from_secs(2), &mut task)
            .await
            .is_err()
        {
            task.abort();
            let _ = task.await;
        }
    }
}

pub(crate) fn spawn_lan_runtime(
    config: LanRuntimeConfig,
    event_tx: mpsc::Sender<LanRuntimeEvent>,
) -> Result<LanTaskHandle, DiscoveryError> {
    let (shutdown_tx, mut shutdown_rx) = broadcast::channel(1);
    let daemon = ServiceDaemon::new().map_err(|error| DiscoveryError::Mdns(error.to_string()))?;
    let service_info = build_service_info(&config)?;
    daemon
        .register(service_info)
        .map_err(|error| DiscoveryError::Mdns(error.to_string()))?;
    let receiver = daemon
        .browse(SERVICE_TYPE)
        .map_err(|error| DiscoveryError::Mdns(error.to_string()))?;

    let task = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => break,
                event = receiver.recv_async() => {
                    let Ok(event) = event else {
                        break;
                    };

                    match event {
                        ServiceEvent::ServiceResolved(service) => {
                            if let Some(record) = parse_resolved(&config, &service) {
                                if event_tx.send(LanRuntimeEvent::Resolved(record)).await.is_err() {
                                    break;
                                }
                            }
                        }
                        ServiceEvent::ServiceRemoved(_, fullname) => {
                            if event_tx.send(LanRuntimeEvent::Removed(fullname)).await.is_err() {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        if let Ok(status_rx) = daemon.shutdown() {
            let _ = status_rx.recv_async().await;
        }
    });

    Ok(LanTaskHandle { shutdown_tx, task })
}

fn build_service_info(config: &LanRuntimeConfig) -> Result<ServiceInfo, DiscoveryError> {
    let instance_name = if config.local_device_id.trim().is_empty() {
        "nooboard".to_string()
    } else {
        sanitize_label(&config.local_device_id)
    };
    let host_name = format!("{}.local.", instance_name);
    let addresses: Vec<IpAddr> = config
        .advertise_addrs
        .iter()
        .copied()
        .map(IpAddr::V4)
        .collect();
    let interfaces = config
        .advertise_addrs
        .iter()
        .copied()
        .map(IpAddr::V4)
        .map(IfKind::Addr)
        .collect::<Vec<_>>();
    let properties = [
        (TXT_PROTOCOL_VERSION, crate::protocol::PROTOCOL_VERSION.to_string()),
        (TXT_NOOB_ID, config.local_noob_id.clone()),
        (TXT_DEVICE_ID, config.local_device_id.clone()),
        (TXT_BOOT_ID, config.boot_id.clone()),
    ];

    let mut info = ServiceInfo::new(
        SERVICE_TYPE,
        &instance_name,
        &host_name,
        addresses.as_slice(),
        config.listen_port,
        properties.as_slice(),
    )
    .map(ServiceInfo::enable_addr_auto)
    .map_err(|error| DiscoveryError::Mdns(error.to_string()))?;
    info.set_interfaces(interfaces);
    Ok(info)
}

fn parse_resolved(
    config: &LanRuntimeConfig,
    service: &mdns_sd::ResolvedService,
) -> Option<LanServiceRecord> {
    let noob_id = service.get_property_val_str(TXT_NOOB_ID)?.to_string();
    if noob_id == config.local_noob_id {
        return None;
    }

    let boot_id = service.get_property_val_str(TXT_BOOT_ID)?.to_string();
    if boot_id == config.boot_id {
        return None;
    }

    let device_id = service.get_property_val_str(TXT_DEVICE_ID)?.to_string();
    let mut addresses: Vec<SocketAddr> = service
        .get_addresses_v4()
        .into_iter()
        .map(|ip| SocketAddr::new(IpAddr::V4(ip), service.get_port()))
        .collect();
    addresses.sort();
    addresses.dedup();
    if addresses.is_empty() {
        return None;
    }

    Some(LanServiceRecord {
        fullname: service.get_fullname().to_string(),
        noob_id,
        device_id,
        addresses,
        last_seen_at_ms: now_millis(),
    })
}

fn sanitize_label(value: &str) -> String {
    let mut sanitized: String = value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-')
        .collect();
    if sanitized.is_empty() {
        sanitized = "nooboard".to_string();
    }
    sanitized
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use mdns_sd::ServiceInfo;

    use super::*;

    fn test_config() -> LanRuntimeConfig {
        LanRuntimeConfig {
            local_noob_id: "local-noob".to_string(),
            local_device_id: "Local Device".to_string(),
            boot_id: "boot-a".to_string(),
            listen_port: 17890,
            advertise_addrs: vec![
                Ipv4Addr::new(10, 0, 0, 2),
                Ipv4Addr::new(10, 0, 0, 3),
            ],
        }
    }

    fn resolved_service(
        noob_id: &str,
        device_id: &str,
        boot_id: &str,
        addrs: &[Ipv4Addr],
        port: u16,
    ) -> mdns_sd::ResolvedService {
        let protocol_version = crate::protocol::PROTOCOL_VERSION.to_string();
        let addresses = addrs.iter().map(ToString::to_string).collect::<Vec<_>>();
        let properties = vec![
            (TXT_PROTOCOL_VERSION.to_string(), protocol_version),
            (TXT_NOOB_ID.to_string(), noob_id.to_string()),
            (TXT_DEVICE_ID.to_string(), device_id.to_string()),
            (TXT_BOOT_ID.to_string(), boot_id.to_string()),
        ];
        ServiceInfo::new(
            SERVICE_TYPE,
            "remote-device",
            "remote-device.local.",
            addresses.as_slice(),
            port,
            properties.as_slice(),
        )
        .expect("service info")
        .as_resolved_service()
    }

    #[test]
    fn build_service_info_advertises_only_configured_ipv4_addrs_and_txt() {
        let config = test_config();
        let service = build_service_info(&config).expect("service");

        let addrs = service.get_addresses_v4();
        assert_eq!(service.get_port(), 17890);
        assert_eq!(addrs.len(), 2);
        assert!(addrs.contains(&Ipv4Addr::new(10, 0, 0, 2)));
        assert!(addrs.contains(&Ipv4Addr::new(10, 0, 0, 3)));
        assert_eq!(service.get_property_val_str(TXT_NOOB_ID), Some("local-noob"));
        assert_eq!(service.get_property_val_str(TXT_DEVICE_ID), Some("Local Device"));
        assert_eq!(service.get_property_val_str(TXT_BOOT_ID), Some("boot-a"));
        let expected_protocol = crate::protocol::PROTOCOL_VERSION.to_string();
        assert_eq!(
            service.get_property_val_str(TXT_PROTOCOL_VERSION),
            Some(expected_protocol.as_str())
        );
    }

    #[test]
    fn parse_resolved_ignores_local_identity_and_boot_id() {
        let config = test_config();
        let same_noob = resolved_service(
            "local-noob",
            "Remote Device",
            "boot-z",
            &[Ipv4Addr::new(10, 0, 0, 10)],
            17890,
        );
        assert!(parse_resolved(&config, &same_noob).is_none());

        let same_boot = resolved_service(
            "remote-noob",
            "Remote Device",
            "boot-a",
            &[Ipv4Addr::new(10, 0, 0, 10)],
            17890,
        );
        assert!(parse_resolved(&config, &same_boot).is_none());
    }

    #[test]
    fn parse_resolved_builds_sorted_ipv4_socket_addrs() {
        let config = test_config();
        let resolved = resolved_service(
            "remote-noob",
            "Remote Device",
            "boot-b",
            &[Ipv4Addr::new(10, 0, 0, 9), Ipv4Addr::new(10, 0, 0, 8)],
            17900,
        );

        let record = parse_resolved(&config, &resolved).expect("record");
        assert_eq!(record.noob_id, "remote-noob");
        assert_eq!(record.device_id, "Remote Device");
        assert_eq!(
            record.addresses,
            vec![
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 8)), 17900),
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9)), 17900),
            ]
        );
        assert_eq!(record.fullname, resolved.get_fullname());
    }
}
