//! Merge serialized settings patches into the latest core configuration.
use crate::{errors, host::Host};
use nooboard_core::{Mode, Settings};
use serde::Deserialize;
use std::{net::SocketAddr, num::NonZeroU16};
use tauri::State;
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsPatch {
    pairing_port: Option<NonZeroU16>,
    discoverable: Option<bool>,
    mode: Option<Mode>,
    receive: Option<bool>,
    paused: Option<bool>,
    history: Option<bool>,
    max_history_entries: Option<u32>,
    history_days: Option<u32>,
    device_name: Option<String>,
}
impl SettingsPatch {
    fn apply(self, mut settings: Settings) -> Result<Settings, crate::errors::UiError> {
        if let Some(port) = self.pairing_port {
            settings.pairing_listen_address = with_port(&settings.pairing_listen_address, port)?;
        }
        if let Some(v) = self.discoverable {
            settings.discoverable = v;
        }
        if let Some(v) = self.mode {
            settings.mode = v;
        }
        if let Some(v) = self.receive {
            settings.receive = v;
        }
        if let Some(v) = self.paused {
            settings.paused = v;
        }
        if let Some(v) = self.history {
            settings.history = v;
        }
        if let Some(v) = self.max_history_entries {
            settings.max_history_entries = v;
        }
        if let Some(v) = self.history_days {
            settings.history_days = v;
        }
        if let Some(v) = self.device_name {
            settings.device_name = v;
        }
        Ok(settings)
    }
}

fn with_port(address: &str, port: NonZeroU16) -> Result<String, crate::errors::UiError> {
    let mut address: SocketAddr = address
        .parse()
        .map_err(|_| crate::errors::ui("configuration"))?;
    address.set_port(port.get());
    Ok(address.to_string())
}
#[tauri::command]
pub async fn desktop_settings(
    host: State<'_, Host>,
    patch: SettingsPatch,
) -> Result<(), crate::errors::UiError> {
    let _mutation = host.mutations.lock().await;
    let app = host.app().await?;
    app.set_settings(patch.apply(app.status().settings)?)
        .await
        .map_err(errors::core)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_pairing_port_and_preserves_sync_listener() {
        let settings = Settings {
            listen_address: "192.168.1.4:24816".into(),
            pairing_listen_address: "[fd00::1]:24817".into(),
            ..Settings::default()
        };
        let patch: SettingsPatch = serde_json::from_str(r#"{"pairingPort":30001}"#).unwrap();
        let updated = patch.apply(settings).unwrap();
        assert_eq!(updated.listen_address, "192.168.1.4:24816");
        assert_eq!(updated.pairing_listen_address, "[fd00::1]:30001");
    }

    #[test]
    fn renaming_keeps_automatic_diagnostic_ports() {
        let settings = Settings {
            listen_address: "127.0.0.1:0".into(),
            pairing_listen_address: "127.0.0.1:0".into(),
            ..Settings::default()
        };
        let patch: SettingsPatch = serde_json::from_str(r#"{"deviceName":"我的电脑"}"#).unwrap();
        let updated = patch.apply(settings).unwrap();
        assert_eq!(updated.listen_address, "127.0.0.1:0");
        assert_eq!(updated.pairing_listen_address, "127.0.0.1:0");
        assert_eq!(updated.device_name, "我的电脑");
    }

    #[test]
    fn rejects_invalid_ports_and_obsolete_address_fields() {
        for value in ["0", "-1", "65536", "2.5", "\"24817\""] {
            assert!(
                serde_json::from_str::<SettingsPatch>(&format!("{{\"pairingPort\":{value}}}"))
                    .is_err()
            );
        }
        assert!(serde_json::from_str::<SettingsPatch>(r#"{"syncPort":30000}"#).is_err());
        for field in ["listenAddress", "pairingListenAddress"] {
            assert!(
                serde_json::from_str::<SettingsPatch>(&format!("{{\"{field}\":\"0.0.0.0:1234\"}}"))
                    .is_err()
            );
        }
    }
}
