//! Per-device configuration and removal.
use crate::{errors, host::Host};
use nooboard_core::PeerSettings;
use serde::Deserialize;
use tauri::State;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PeerPatch {
    address: Option<String>,
    auto_send: Option<bool>,
    has_address: bool,
}
#[tauri::command]
pub async fn desktop_peer_settings(
    host: State<'_, Host>,
    noob_id: String,
    patch: PeerPatch,
) -> Result<(), crate::errors::UiError> {
    let _mutation = host.mutations.lock().await;
    let app = host.app().await?;
    let peer = app
        .status()
        .peers
        .into_iter()
        .find(|p| p.noob_id == noob_id)
        .ok_or(crate::errors::ui("peerRemoved"))?;
    let settings = PeerSettings {
        address: if patch.has_address {
            patch.address
        } else {
            peer.settings.address
        },
        auto_send: patch.auto_send.unwrap_or(peer.settings.auto_send),
    };
    app.configure_peer(noob_id, settings)
        .await
        .map_err(errors::core)
}
#[tauri::command]
pub async fn desktop_select_targets(
    host: State<'_, Host>,
    targets: Vec<String>,
) -> Result<(), crate::errors::UiError> {
    host.app()
        .await?
        .select_targets(targets)
        .await
        .map_err(errors::core)
}
#[tauri::command]
pub async fn desktop_unpair(
    host: State<'_, Host>,
    noob_id: String,
) -> Result<(), crate::errors::UiError> {
    host.app()
        .await?
        .unpair(noob_id)
        .await
        .map_err(errors::core)
}
