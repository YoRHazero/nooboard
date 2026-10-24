use crate::{
    app::runtime::Runtime,
    ipc::{
        contracts::*,
        errors::{self, UiError},
        mapping::settings,
    },
};
use nooboard_core::App;
pub async fn sync(
    runtime: &Runtime,
    app: &App,
    patch: SyncSettingsPatch,
) -> Result<Reply, UiError> {
    let revision = runtime
        .backend
        .edit_settings(app, |value| Ok(settings::sync(value, patch)))
        .await
        .map_err(errors::desktop)?;
    Ok(Reply::Saved {
        revision: revision.to_string(),
    })
}
pub async fn local(
    runtime: &Runtime,
    app: &App,
    patch: LocalDevicePatch,
) -> Result<Reply, UiError> {
    let revision = runtime
        .backend
        .edit_settings(app, |value| settings::local(value, patch))
        .await
        .map_err(errors::desktop)?;
    Ok(Reply::Saved {
        revision: revision.to_string(),
    })
}
pub async fn peer(
    runtime: &Runtime,
    app: &App,
    id: String,
    patch: PeerPatch,
) -> Result<(), UiError> {
    runtime
        .backend
        .edit_peer(app, id, |mut settings| {
            if let Some(address) = patch.address {
                settings.address = match address {
                    AddressChange::Clear => None,
                    AddressChange::Set(value) => Some(value),
                };
            }
            if let Some(value) = patch.auto_send {
                settings.auto_send = value;
            }
            settings
        })
        .await
        .map_err(|error| match error {
            crate::error::Error::Core(nooboard_core::Error::NotFound) => errors::ui("peerRemoved"),
            other => errors::desktop(other),
        })
}
