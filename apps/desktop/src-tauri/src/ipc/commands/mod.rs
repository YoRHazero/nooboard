mod settings;
use super::{
    dto::*,
    errors::{self, UiError},
};
use crate::host::Host;
use tauri::{AppHandle, Manager};

pub async fn run(handle: &AppHandle, host: &Host, request: Request) -> Result<Reply, UiError> {
    match request {
        Request::HostPreferences {
            patch,
            legacy_language,
        } => {
            return crate::desktop::desktop_preferences(
                handle.clone(),
                handle.state(),
                Some(patch),
                legacy_language,
            )
            .map(Reply::Host);
        }
        Request::AcknowledgeNavigation { id } => {
            crate::desktop::desktop_navigation_ack(handle.state(), id);
            return Ok(Reply::Done);
        }
        Request::Probe { action } => {
            #[cfg(all(debug_assertions, feature = "diagnostics", target_os = "macos"))]
            if let Some(crate::host::Running::Diagnostic(diagnostic)) =
                host.running.write().await.as_mut()
            {
                diagnostic
                    .action(match action {
                        ProbeAction::Copy => "copy",
                        ProbeAction::Receive => "receive",
                    })
                    .await?;
                return Ok(Reply::Done);
            }
            let _ = action;
            return Err(errors::ui("diagnosticsDisabled"));
        }
        _ => {}
    }
    let app = host.app().await?;
    match request {
        Request::Discover => app.refresh_discovery().await,
        Request::BeginPairing { address, expected } => app.begin_pairing(address, expected).await,
        Request::AcceptPairing { id } => app.accept_pairing(id).await,
        Request::PairingCode { id, code } => app.submit_pairing_code(id, code).await,
        Request::DismissPairing { id } => app.dismiss_pairing(id).await,
        Request::UpdateSyncSettings { patch } => {
            let _mutation = host.mutations.lock().await;
            app.set_settings(settings::sync(app.status().settings, patch))
                .await
                .map_err(errors::core)?;
            return Ok(Reply::Saved {
                revision: app.status().configuration_revision.to_string(),
            });
        }
        Request::UpdateLocalDevice { patch } => {
            let _mutation = host.mutations.lock().await;
            app.set_settings(settings::local(app.status().settings, patch)?)
                .await
                .map_err(errors::core)?;
            return Ok(Reply::Saved {
                revision: app.status().configuration_revision.to_string(),
            });
        }
        Request::ConfigurePeer { noob_id, patch } => {
            let _mutation = host.mutations.lock().await;
            let mut settings = app
                .status()
                .peers
                .into_iter()
                .find(|p| p.noob_id == noob_id)
                .ok_or(errors::ui("peerRemoved"))?
                .settings;
            if let Some(address) = patch.address {
                settings.address = match address {
                    AddressChange::Clear => None,
                    AddressChange::Set(value) => Some(value),
                };
            }
            if let Some(value) = patch.auto_send {
                settings.auto_send = value;
            }
            app.configure_peer(noob_id, settings).await
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
        Request::SelectFiles => {
            let Some(files) = rfd::AsyncFileDialog::new().pick_files().await else {
                return Ok(Reply::Done);
            };
            app.send_files(files.iter().map(|f| f.path().to_owned()).collect())
                .await
        }
        Request::SelectReceiveDirectory => {
            let Some(directory) = rfd::AsyncFileDialog::new().pick_folder().await else {
                return Ok(Reply::Done);
            };
            let _mutation = host.mutations.lock().await;
            let mut settings = app.status().settings;
            settings.receive_directory = Some(directory.path().to_owned());
            app.set_settings(settings).await
        }
        Request::CancelTransfer { key } => app.cancel_transfer(key).await,
        Request::CopyReceived { key } => app.copy_received(key).await,
        Request::QueryHistory { query } => {
            if query.contains.len() > 4096 {
                return Err(errors::ui("invalidHistoryQuery"));
            }
            let local = match query.source {
                HistorySource::All => None,
                HistorySource::Local => Some(true),
                HistorySource::Remote => Some(false),
            };
            let mut rows = app
                .history_filtered(query.contains, local, 33, query.offset)
                .await
                .map_err(errors::core)?;
            let has_more = rows.len() > 32;
            rows.truncate(32);
            return Ok(Reply::History(HistoryPage {
                items: rows.into_iter().map(Into::into).collect(),
                has_more,
            }));
        }
        Request::CopyHistory { id } => app.copy_history(history_id(&id)?).await,
        Request::DeleteHistory { id } => app.delete_history(history_id(&id)?).await,
        Request::ClearHistory => app.clear_history().await,
        Request::HostPreferences { .. }
        | Request::AcknowledgeNavigation { .. }
        | Request::Probe { .. } => unreachable!("handled before obtaining core handle"),
    }
    .map_err(errors::core)?;
    Ok(Reply::Done)
}
fn history_id(id: &str) -> Result<i64, UiError> {
    id.parse()
        .ok()
        .filter(|id| *id > 0)
        .ok_or(errors::ui("invalidHistoryAction"))
}
