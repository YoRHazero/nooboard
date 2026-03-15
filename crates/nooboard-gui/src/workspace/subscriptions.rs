use gpui::{AsyncApp, WeakEntity};
use nooboard_core::{EventSubscription, StateSubscription};

use super::{controller::WorkspaceController, core_bridge::CoreBridge};

pub struct WorkspaceSubscriptions {
    pub state: StateSubscription,
    pub events: EventSubscription,
}

pub fn spawn_state_bridge(
    controller: WeakEntity<WorkspaceController>,
    bridge: CoreBridge,
    mut subscription: StateSubscription,
    cx: &AsyncApp,
) {
    cx.spawn(async move |cx| {
        let mut latest_record_event_id = subscription.latest().clipboard.latest_committed_event_id;

        loop {
            let next_snapshot = match subscription.recv().await {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    let _ = controller.update(cx, |this, cx| {
                        this.mark_state_stream_closed(error.to_string());
                        cx.notify();
                    });
                    break;
                }
            };

            let next_record_event_id = next_snapshot.clipboard.latest_committed_event_id;
            let refresh_latest_record = next_record_event_id != latest_record_event_id;
            latest_record_event_id = next_record_event_id;

            let _ = controller.update(cx, |this, cx| {
                this.apply_snapshot(next_snapshot.clone());
                cx.notify();
            });

            if !refresh_latest_record {
                continue;
            }

            match bridge.fetch_latest_committed_record(&next_snapshot).await {
                Ok(record) => {
                    let _ = controller.update(cx, |this, cx| {
                        this.replace_latest_committed_record(record);
                        cx.notify();
                    });
                }
                Err(error) => {
                    let _ = controller.update(cx, |this, cx| {
                        this.record_bridge_warning(format!(
                            "failed to refresh latest clipboard record: {error}"
                        ));
                        cx.notify();
                    });
                }
            }
        }

        Ok::<_, anyhow::Error>(())
    })
    .detach();
}

pub fn spawn_event_bridge(
    controller: WeakEntity<WorkspaceController>,
    _bridge: CoreBridge,
    mut subscription: EventSubscription,
    cx: &AsyncApp,
) {
    cx.spawn(async move |cx| {
        loop {
            let event = match subscription.recv().await {
                Ok(event) => event,
                Err(error) => {
                    let _ = controller.update(cx, |this, cx| {
                        this.mark_event_stream_closed(error.to_string());
                        cx.notify();
                    });
                    break;
                }
            };

            let _ = controller.update(cx, |this, cx| {
                this.apply_event(event);
                cx.notify();
            });
        }

        Ok::<_, anyhow::Error>(())
    })
    .detach();
}
