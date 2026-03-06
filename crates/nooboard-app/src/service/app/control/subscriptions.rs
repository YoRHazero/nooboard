use crate::AppResult;
use crate::clipboard_runtime::LocalClipboardSubscription;
use crate::service::types::EventSubscription;

use super::state::ControlState;

pub(super) async fn subscribe_events(state: &ControlState) -> AppResult<EventSubscription> {
    state.subscriptions.subscribe().await
}

pub(super) fn subscribe_local_clipboard(
    state: &ControlState,
) -> AppResult<LocalClipboardSubscription> {
    state.clipboard.subscribe_local_changes()
}
