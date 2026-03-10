use anyhow::Error;
use gpui::{AsyncApp, Context, Entity};
use nooboard_app::{
    AppError, DesktopAppService, EventId, IncomingTransferDecision, SendFilesRequest,
    SettingsPatch, SyncDesiredState, TransferId,
};

use super::live_app::{DesktopLiveApp, LiveAppStore};

#[derive(Clone)]
pub struct LiveCommandClient {
    live_app: DesktopLiveApp,
    store: Entity<LiveAppStore>,
}

impl LiveCommandClient {
    fn record_desktop_warning(&self, cx: &mut AsyncApp, message: String) {
        let _ = self.store.update(cx, |store, cx| {
            store.record_desktop_warning(message);
            cx.notify();
        });
    }

    pub async fn set_sync_desired_state(
        &self,
        desired: SyncDesiredState,
        cx: &mut AsyncApp,
    ) -> Result<(), AppError> {
        match self
            .live_app
            .service()
            .set_sync_desired_state(desired)
            .await
        {
            Ok(()) => Ok(()),
            Err(error) => {
                self.record_desktop_warning(
                    cx,
                    format!("failed to set sync desired state to {:?}: {error}", desired),
                );
                Err(error)
            }
        }
    }

    pub async fn adopt_clipboard_record(
        &self,
        event_id: EventId,
        cx: &mut AsyncApp,
    ) -> Result<(), AppError> {
        match self
            .live_app
            .service()
            .adopt_clipboard_record(event_id)
            .await
        {
            Ok(()) => Ok(()),
            Err(error) => {
                self.record_desktop_warning(
                    cx,
                    format!("failed to adopt clipboard record {event_id}: {error}"),
                );
                Err(error)
            }
        }
    }

    pub async fn send_files(
        &self,
        request: SendFilesRequest,
        cx: &mut AsyncApp,
    ) -> Result<Vec<TransferId>, AppError> {
        match self.live_app.service().send_files(request).await {
            Ok(transfer_ids) => Ok(transfer_ids),
            Err(error) => {
                self.record_desktop_warning(cx, format!("failed to send files: {error}"));
                Err(error)
            }
        }
    }

    pub async fn decide_incoming_transfer(
        &self,
        request: IncomingTransferDecision,
        cx: &mut AsyncApp,
    ) -> Result<(), AppError> {
        match self
            .live_app
            .service()
            .decide_incoming_transfer(request.clone())
            .await
        {
            Ok(()) => Ok(()),
            Err(error) => {
                self.record_desktop_warning(
                    cx,
                    format!(
                        "failed to decide incoming transfer {}: {error}",
                        request.transfer_id
                    ),
                );
                Err(error)
            }
        }
    }

    pub async fn cancel_transfer(
        &self,
        transfer_id: TransferId,
        cx: &mut AsyncApp,
    ) -> Result<(), AppError> {
        match self
            .live_app
            .service()
            .cancel_transfer(transfer_id.clone())
            .await
        {
            Ok(()) => Ok(()),
            Err(error) => {
                self.record_desktop_warning(
                    cx,
                    format!("failed to cancel transfer {transfer_id}: {error}"),
                );
                Err(error)
            }
        }
    }

    pub async fn patch_settings(
        &self,
        patch: SettingsPatch,
        cx: &mut AsyncApp,
    ) -> Result<(), AppError> {
        match self.live_app.service().patch_settings(patch.clone()).await {
            Ok(()) => Ok(()),
            Err(error) => {
                self.record_desktop_warning(
                    cx,
                    format!("failed to patch settings {patch:?}: {error}"),
                );
                Err(error)
            }
        }
    }
}

pub fn client<T: 'static>(cx: &mut Context<T>) -> LiveCommandClient {
    let live_app = cx.global::<DesktopLiveApp>().clone();
    let store = live_app.store();

    LiveCommandClient { live_app, store }
}

pub fn set_sync_desired_state<T: 'static>(desired: SyncDesiredState, cx: &mut Context<T>) {
    let commands = client(cx);

    cx.spawn(async move |_, cx| {
        let _ = commands.set_sync_desired_state(desired, cx).await;
        Ok::<_, Error>(())
    })
    .detach();
}

pub fn adopt_clipboard_record<T: 'static>(event_id: EventId, cx: &mut Context<T>) {
    let commands = client(cx);

    cx.spawn(async move |_, cx| {
        let _ = commands.adopt_clipboard_record(event_id, cx).await;
        Ok::<_, Error>(())
    })
    .detach();
}
