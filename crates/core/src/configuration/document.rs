//! Application document versions are independent of database schema versions.
use super::model::{Configuration, Peer};
use crate::{Error, PeerSettings, Result, Settings};
use nooboard_storage::{Setting, SettingsChanges, Storage};
use serde::Deserialize;
use std::collections::BTreeMap;

pub(crate) const KEY: &str = "core.configuration";
const LEGACY_KEY: &str = "configuration_v2";

pub(crate) async fn load(storage: &Storage, defaults: Settings) -> Result<Configuration> {
    let documents = storage
        .settings()
        .get_many(vec![
            KEY.into(),
            LEGACY_KEY.into(),
            "settings".into(),
            "peer".into(),
        ])
        .await?;
    let default_receive_directory = defaults.receive_directory.clone();
    let (mut config, migrated) = if let Some(text) = &documents[0] {
        (
            serde_json::from_str(text).map_err(|_| Error::Configuration)?,
            false,
        )
    } else if let Some(text) = &documents[1] {
        (migrate_v2(text)?, true)
    } else if documents[3].is_some() {
        // A pre-registry single-peer document needs an explicit import; never
        // silently start with a different device registry or discard its certificate.
        return Err(Error::Configuration);
    } else {
        let settings = documents[2]
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .map_err(|_| Error::Configuration)?
            .unwrap_or(defaults);
        (Configuration::new(settings), documents[2].is_some())
    };
    if config.settings.receive_directory.is_none() {
        config.settings.receive_directory = default_receive_directory;
    }
    config.validate()?;
    write(
        storage,
        &config,
        if migrated {
            vec![LEGACY_KEY.into(), "settings".into(), "peer".into()]
        } else {
            vec![]
        },
    )
    .await?;
    Ok(config)
}

fn migrate_v2(text: &str) -> Result<Configuration> {
    #[derive(Deserialize)]
    struct LegacyPeer {
        noob_id: String,
        device_name: String,
        certificate: Vec<u8>,
        settings: PeerSettings,
    }
    #[derive(Deserialize)]
    struct Legacy {
        version: u16,
        settings: Settings,
        peers: BTreeMap<String, LegacyPeer>,
        manual_targets: Vec<String>,
    }
    let old: Legacy = serde_json::from_str(text).map_err(|_| Error::Configuration)?;
    if old.version != 2 {
        return Err(Error::Configuration);
    }
    let mut config = Configuration::new(old.settings);
    config.manual_targets = old.manual_targets;
    for (id, peer) in old.peers {
        let identity =
            nooboard_network::PublicIdentity::from_certificate(peer.device_name, peer.certificate)?;
        if id != peer.noob_id || id != identity.id {
            return Err(Error::Configuration);
        }
        // Hostnames are resolved by devices; configuration only interprets its document.
        let addresses = peer
            .settings
            .address
            .as_deref()
            .and_then(|address| address.parse().ok())
            .into_iter()
            .collect();
        config.peers.insert(
            id,
            Peer {
                trusted: nooboard_network::TrustedPeer {
                    identity,
                    addresses,
                },
                settings: peer.settings,
            },
        );
    }
    config.validate()?;
    Ok(config)
}

pub(crate) async fn save(storage: &Storage, config: &Configuration) -> Result<()> {
    write(storage, config, vec![]).await
}
async fn write(storage: &Storage, config: &Configuration, delete: Vec<String>) -> Result<()> {
    let value = serde_json::to_string(config).map_err(|_| Error::Configuration)?;
    storage
        .settings()
        .apply(SettingsChanges {
            put: vec![Setting {
                key: KEY.into(),
                value,
            }],
            delete,
        })
        .await?;
    Ok(())
}
