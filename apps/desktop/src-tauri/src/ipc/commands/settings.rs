use crate::ipc::{
    dto::{LocalDevicePatch, SendMode, SyncSettingsPatch},
    errors::{self, UiError},
};
use nooboard_core::{Mode, Settings};
pub fn sync(mut settings: Settings, patch: SyncSettingsPatch) -> Settings {
    if let Some(v) = patch.discoverable {
        settings.discoverable = v;
    }
    if let Some(v) = patch.mode {
        settings.mode = match v {
            SendMode::Manual => Mode::Manual,
            SendMode::Automatic => Mode::Automatic,
        };
    }
    if let Some(v) = patch.receive {
        settings.receive = v;
    }
    if let Some(v) = patch.paused {
        settings.paused = v;
    }
    if let Some(v) = patch.history {
        settings.history = v;
    }
    if let Some(v) = patch.max_history_entries {
        settings.max_history_entries = v;
    }
    if let Some(v) = patch.history_days {
        settings.history_days = v;
    }
    settings
}
pub fn local(mut settings: Settings, patch: LocalDevicePatch) -> Result<Settings, UiError> {
    if let Some(v) = patch.device_name {
        settings.device_name = v;
    }
    if let Some(port) = patch.pairing_port {
        let mut address: std::net::SocketAddr = settings
            .pairing_listen_address
            .parse()
            .map_err(|_| errors::ui("configuration"))?;
        address.set_port(port.get());
        settings.pairing_listen_address = address.to_string();
    }
    Ok(settings)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn patch_preserves_unrelated_settings_and_listener_address() {
        let settings = Settings {
            listen_address: "192.168.1.4:24816".into(),
            pairing_listen_address: "[fd00::1]:24817".into(),
            ..Settings::default()
        };
        let patch =
            serde_json::from_str(r#"{"pairingPort":30001,"deviceName":"我的电脑"}"#).unwrap();
        let updated = local(settings, patch).unwrap();
        assert_eq!(updated.listen_address, "192.168.1.4:24816");
        assert_eq!(updated.pairing_listen_address, "[fd00::1]:30001");
        assert_eq!(updated.device_name, "我的电脑");
        assert_eq!(updated.history_days, 30);
    }
    #[test]
    fn invalid_ports_and_unknown_fields_are_rejected() {
        for value in ["0", "-1", "65536", "2.5", "\"24817\""] {
            assert!(
                serde_json::from_str::<LocalDevicePatch>(&format!("{{\"pairingPort\":{value}}}"))
                    .is_err()
            );
        }
        assert!(serde_json::from_str::<LocalDevicePatch>(r#"{"syncPort":30000}"#).is_err());
    }
}
