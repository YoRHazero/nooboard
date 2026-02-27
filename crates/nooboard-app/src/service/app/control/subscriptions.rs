use crate::AppResult;
use crate::service::types::EventSubscription;

use super::state::ControlState;

pub(super) async fn subscribe_events(state: &ControlState) -> AppResult<EventSubscription> {
    state.subscriptions.subscribe().await
}
