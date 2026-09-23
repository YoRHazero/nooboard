use super::{dto::*, errors};

pub fn snapshot(value: nooboard_core::AppSnapshot) -> BackendSnapshot {
    let status = value.status;
    let settings = status.settings;
    BackendSnapshot {
        session: value.session,
        revision: value.revision.to_string(),
        history_revision: value.history_revision.to_string(),
        state: status.state.into(),
        configuration: ConfigurationStatus {
            saved_revision: status.configuration_revision.to_string(),
            effective_revision: status.effective_revision.to_string(),
            restart_required: status.restart_required,
        },
        local_device: LocalDevice {
            noob_id: status.noob_id,
            device_name: settings.device_name,
            fingerprint: status.fingerprint,
            platform: match std::env::consts::OS {
                "macos" => "macOS",
                "windows" => "Windows",
                _ => "Linux",
            }
            .into(),
            sync_port: value.local_network.sync_port,
            pairing_port: value.local_network.pairing_port,
            addresses: value
                .local_network
                .addresses
                .into_iter()
                .map(|a| LocalAddress {
                    interface: a.interface,
                    ip: a.ip.to_string(),
                    pairing_address: a.pairing_address.to_string(),
                })
                .collect(),
            address_error: value
                .local_network
                .error
                .map(|_| errors::ui("localAddress")),
        },
        settings: SyncSettings {
            receive_directory: settings
                .receive_directory
                .map(|p| p.to_string_lossy().into_owned()),
            discoverable: settings.discoverable,
            mode: match settings.mode {
                nooboard_core::Mode::Manual => SendMode::Manual,
                nooboard_core::Mode::Automatic => SendMode::Automatic,
            },
            receive: settings.receive,
            paused: settings.paused,
            history: settings.history,
            max_history_entries: settings.max_history_entries,
            history_days: settings.history_days,
        },
        current: Clipboard {
            revision: value.current.revision.to_string(),
            kind: value.current.kind.into(),
            text: value.current.text,
            source: value.current.source,
            copied_at: value.current.copied_at_ms,
            files: value.current.files,
            preview: value.current.preview,
            image_width: value.current.image_width,
            image_height: value.current.image_height,
        },
        peers: status
            .peers
            .into_iter()
            .map(|p| Peer {
                noob_id: p.noob_id,
                device_name: p.device_name,
                fingerprint: p.fingerprint,
                settings: PeerSettings {
                    address: p.settings.address,
                    auto_send: p.settings.auto_send,
                },
                online: p.online,
                accepting: p.accepting,
            })
            .collect(),
        manual_targets: status.manual_targets,
        transfers: status
            .transfers
            .into_iter()
            .map(|t| Transfer {
                id: t.id.into(),
                automatic: t.automatic,
                bytes: t.bytes,
                targets: t
                    .targets
                    .into_iter()
                    .map(|d| Delivery {
                        noob_id: d.noob_id,
                        device_name: d.device_name,
                        state: d.state.into(),
                    })
                    .collect(),
            })
            .collect(),
        content_transfers: value
            .content_transfers
            .into_iter()
            .map(|t| ContentTransfer {
                key: t.key,
                id: t.id.into(),
                peer: t.peer,
                device_name: t.device_name,
                incoming: t.incoming,
                kind: t.kind.into(),
                names: t.names,
                total_bytes: t.total_bytes,
                completed_bytes: t.completed_bytes,
                stage: t.stage.into(),
                error: t.error.map(Into::into),
                saved_paths: t
                    .saved_paths
                    .into_iter()
                    .map(|p| p.to_string_lossy().into_owned())
                    .collect(),
                at: t.at_ms,
            })
            .collect(),
        onboarding: Onboarding {
            nearby: value
                .onboarding
                .nearby
                .into_iter()
                .map(|p| NearbyDevice {
                    key: p.key,
                    noob_id: p.noob_id,
                    device_name: p.device_name,
                    addresses: p.addresses.into_iter().map(|a| a.to_string()).collect(),
                    sync_port: p.sync_port,
                })
                .collect(),
            error: value
                .onboarding
                .discovery_error
                .map(|_| errors::ui("discovery")),
            session: value.onboarding.session.map(|p| PairingSession {
                id: p.id,
                incoming: p.incoming,
                device_name: p.device_name,
                noob_id: p.noob_id,
                stage: p.stage.into(),
                code: p.code,
                expires_at: p.expires_at_ms,
                attempts_left: p.attempts_left,
                error: p.error.as_ref().map(errors::pairing),
            }),
        },
        activities: value
            .activities
            .into_iter()
            .map(|a| ActivityRecord {
                sequence: a.sequence.to_string(),
                kind: a.kind.into(),
                summary: a.summary,
                at: a.at_ms,
                source: a.source,
                device_name: a.device_name,
                message_id: a.message_id.map(Into::into),
                content_task: a.content_task,
                content_node: a.content_node,
                content_stage: a.content_stage.map(Into::into),
            })
            .collect(),
        fault: value.fault.map(|f| Fault {
            sequence: f.sequence.to_string(),
            message: errors::ui("serviceFault"),
        }),
    }
}

impl From<nooboard_core::HistoryEntry> for HistoryItem {
    fn from(row: nooboard_core::HistoryEntry) -> Self {
        Self {
            id: row.id.to_string(),
            text: row.text,
            source: row.source,
            copied_at: row.copied_at_ms,
        }
    }
}
