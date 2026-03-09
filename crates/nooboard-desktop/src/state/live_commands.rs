use anyhow::Error;
use gpui::Context;
use nooboard_app::{DesktopAppService, EventId, SyncDesiredState};

use super::live_app::DesktopLiveApp;

pub fn set_sync_desired_state<T: 'static>(desired: SyncDesiredState, cx: &mut Context<T>) {
    let live_app = cx.global::<DesktopLiveApp>().clone();
    let store = live_app.store();

    cx.spawn(async move |_, cx| {
        if let Err(error) = live_app.service().set_sync_desired_state(desired).await {
            let _ = store.update(cx, |store, cx| {
                store.record_desktop_warning(format!(
                    "failed to set sync desired state to {:?}: {error}",
                    desired
                ));
                cx.notify();
            });
        }

        Ok::<_, Error>(())
    })
    .detach();
}

pub fn adopt_clipboard_record<T: 'static>(event_id: EventId, cx: &mut Context<T>) {
    let live_app = cx.global::<DesktopLiveApp>().clone();
    let store = live_app.store();

    cx.spawn(async move |_, cx| {
        if let Err(error) = live_app.service().adopt_clipboard_record(event_id).await {
            let _ = store.update(cx, |store, cx| {
                store.record_desktop_warning(format!(
                    "failed to adopt clipboard record {event_id}: {error}"
                ));
                cx.notify();
            });
        }

        Ok::<_, Error>(())
    })
    .detach();
}
