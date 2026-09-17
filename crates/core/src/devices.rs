//! Persisted device registry. Live sessions and transfer outcomes do not belong here.
use crate::{Error, PeerSettings, Result, Settings, VerifiedPeer, ports::Store};
use nooboard_network::{Identity, TlsConfig, fingerprint, noob_id, valid_device_name};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

pub(crate) const MAX_PEERS: usize = 64;
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Peer {
    pub noob_id: String,
    pub device_name: String,
    pub certificate: Vec<u8>,
    pub settings: PeerSettings,
}
impl Peer {
    pub fn confirmed(request: VerifiedPeer, identity: &Identity) -> Result<Self> {
        if request.certificate.len() > 8192
            || fingerprint(&request.certificate) != request.confirmed_fingerprint.to_lowercase()
        {
            return Err(Error::Fingerprint);
        }
        let id = noob_id(&request.certificate)?;
        if id == identity.noob_id()? || !valid_device_name(&request.device_name) {
            return Err(Error::Configuration);
        }
        TlsConfig::new(identity, &request.certificate)?;
        let settings = PeerSettings {
            address: request.address,
            auto_send: false,
        };
        settings.validate()?;
        Ok(Self {
            noob_id: id,
            device_name: request.device_name,
            certificate: request.certificate,
            settings,
        })
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Configuration {
    version: u16,
    pub settings: Settings,
    pub peers: BTreeMap<String, Peer>,
    pub manual_targets: Vec<String>,
}
impl Configuration {
    pub async fn load(
        store: &Store,
        identity: &Identity,
        default_receive_directory: Option<PathBuf>,
    ) -> Result<Self> {
        let (current, settings, legacy) = store
            .run(|db| {
                Ok((
                    db.setting("configuration_v2")?,
                    db.setting("settings")?,
                    db.setting("peer")?,
                ))
            })
            .await?;
        let mut configuration = if let Some(bytes) = current {
            serde_json::from_slice::<Self>(&bytes).map_err(|_| Error::Configuration)?
        } else {
            let mut settings: Settings = settings
                .map(|bytes| serde_json::from_slice(&bytes))
                .transpose()
                .map_err(|_| Error::Configuration)?
                .unwrap_or_else(|| {
                    let name = std::env::var("COMPUTERNAME")
                        .or_else(|_| std::env::var("HOSTNAME"))
                        .ok()
                        .or_else(|| {
                            std::process::Command::new("hostname")
                                .output()
                                .ok()
                                .filter(|o| o.status.success())
                                .and_then(|o| String::from_utf8(o.stdout).ok())
                                .map(|n| n.trim().to_owned())
                        })
                        .filter(|n| valid_device_name(n))
                        .unwrap_or_else(|| "Nooboard".into());
                    Settings {
                        device_name: name,
                        ..Settings::default()
                    }
                });
            let mut peers = BTreeMap::new();
            let mut manual_targets = Vec::new();
            if let Some(bytes) = legacy {
                #[derive(Deserialize)]
                enum LegacyEndpoint {
                    Listen(String),
                    Connect(String),
                }
                #[derive(Deserialize)]
                struct LegacyPeer {
                    certificate: Vec<u8>,
                    endpoint: LegacyEndpoint,
                }
                let old: LegacyPeer =
                    serde_json::from_slice(&bytes).map_err(|_| Error::Configuration)?;
                let address = match old.endpoint {
                    LegacyEndpoint::Listen(a) => {
                        settings.listen_address = a;
                        None
                    }
                    LegacyEndpoint::Connect(a) => Some(a),
                };
                let id = noob_id(&old.certificate)?;
                // Preserve the user's existing one-peer automatic-sync selection.
                peers.insert(
                    id.clone(),
                    Peer {
                        noob_id: id.clone(),
                        device_name: "已配对设备".into(),
                        certificate: old.certificate,
                        settings: PeerSettings {
                            address,
                            auto_send: true,
                        },
                    },
                );
                manual_targets.push(id);
            }
            Self {
                version: 2,
                settings,
                peers,
                manual_targets,
            }
        };
        if configuration.settings.receive_directory.is_none() {
            configuration.settings.receive_directory = default_receive_directory;
        }
        configuration.validate(identity)?;
        configuration.save(store).await?;
        Ok(configuration)
    }
    pub fn validate(&self, identity: &Identity) -> Result<()> {
        if self.version != 2 || self.peers.len() > MAX_PEERS {
            return Err(Error::Configuration);
        }
        self.settings.validate()?;
        for (id, p) in &self.peers {
            if *id != noob_id(&p.certificate)?
                || *id != p.noob_id
                || *id == identity.noob_id()?
                || !valid_device_name(&p.device_name)
            {
                return Err(Error::Configuration);
            }
            p.settings.validate()?;
            TlsConfig::new(identity, &p.certificate)?;
        }
        self.targets(&self.manual_targets)?;
        Ok(())
    }
    pub fn targets(&self, targets: &[String]) -> Result<()> {
        let mut seen = std::collections::BTreeSet::new();
        if targets
            .iter()
            .any(|id| !self.peers.contains_key(id) || !seen.insert(id))
        {
            return Err(Error::Configuration);
        }
        Ok(())
    }
    pub async fn save(&self, store: &Store) -> Result<()> {
        let bytes = serde_json::to_vec(self).map_err(|_| Error::Configuration)?;
        store
            .run(move |db| db.replace_settings("configuration_v2", &bytes, &["settings", "peer"]))
            .await
    }
    pub fn tls(&self, identity: &Identity) -> Result<Option<TlsConfig>> {
        if self.peers.is_empty() {
            return Ok(None);
        }
        Ok(Some(TlsConfig::with_peers(
            identity,
            &self
                .peers
                .values()
                .map(|p| p.certificate.clone())
                .collect::<Vec<_>>(),
        )?))
    }
}
