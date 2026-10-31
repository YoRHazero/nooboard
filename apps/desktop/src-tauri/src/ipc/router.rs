//! Input dispatch and wire replies. Native interaction workflows live in features.
use super::{
    contracts::*,
    errors::{self, UiError},
    mapping, routes,
};
use crate::app::runtime::Runtime;
pub async fn run(runtime: &Runtime, request: Request) -> Result<Reply, UiError> {
    if runtime.lifecycle.control.exiting() {
        return Err(errors::ui("closing"));
    }
    match request {
        Request::InstallUpdate => {
            runtime.install_update().await.map_err(errors::desktop)?;
            return Ok(Reply::Done);
        }
        Request::HostPreferences {
            patch,
            legacy_language,
        } => {
            return runtime
                .update_preferences(patch, legacy_language)
                .map(mapping::desktop::snapshot)
                .map(Reply::Host)
                .map_err(errors::desktop);
        }
        Request::AcknowledgeNavigation { id } => {
            runtime.window.acknowledge(id);
            runtime.publish();
            return Ok(Reply::Done);
        }
        Request::Probe { action } => {
            runtime
                .backend
                .probe(match action {
                    ProbeAction::Copy => "copy",
                    ProbeAction::Receive => "receive",
                })
                .await
                .map_err(errors::desktop)?;
            return Ok(Reply::Done);
        }
        Request::SelectFiles => {
            runtime.select_files().await.map_err(errors::desktop)?;
            return Ok(Reply::Done);
        }
        Request::SelectReceiveDirectory => {
            runtime
                .select_receive_directory()
                .await
                .map_err(errors::desktop)?;
            return Ok(Reply::Done);
        }
        _ => {}
    }
    let app = runtime.backend.app().await.map_err(errors::desktop)?;
    match request {
        Request::Discover => app.refresh_discovery().await,
        Request::BeginPairing { address, expected } => app.begin_pairing(address, expected).await,
        Request::AcceptPairing { id } => app.accept_pairing(id).await,
        Request::PairingCode { id, code } => app.submit_pairing_code(id, code).await,
        Request::DismissPairing { id } => app.dismiss_pairing(id).await,
        Request::UpdateSyncSettings { patch } => {
            return routes::settings::sync(runtime, &app, patch).await;
        }
        Request::UpdateLocalDevice { patch } => {
            return routes::settings::local(runtime, &app, patch).await;
        }
        Request::ConfigurePeer { noob_id, patch } => {
            routes::settings::peer(runtime, &app, noob_id, patch).await?;
            return Ok(Reply::Done);
        }
        Request::SelectTargets { targets } => app.select_targets(targets).await,
        Request::Unpair { noob_id } => app.unpair(noob_id).await,
        Request::SendCurrent => {
            return app
                .send_current()
                .await
                .map(|id| Reply::Sent(id.into()))
                .map_err(errors::core);
        }
        Request::CancelTransfer { key } => app.cancel_transfer(key).await,
        Request::CopyReceived { key } => app.copy_received(key).await,
        Request::QueryHistory { query } => return routes::history::query(&app, query).await,
        Request::CopyHistory { id } => app.copy_history(routes::history::id(&id)?).await,
        Request::DeleteHistory { id } => app.delete_history(routes::history::id(&id)?).await,
        Request::ClearHistory => app.clear_history().await,
        Request::Terminal { .. }
        | Request::InstallUpdate
        | Request::HostPreferences { .. }
        | Request::AcknowledgeNavigation { .. }
        | Request::Probe { .. }
        | Request::SelectFiles
        | Request::SelectReceiveDirectory => unreachable!("handled at the desktop boundary"),
    }
    .map_err(errors::core)?;
    Ok(Reply::Done)
}
