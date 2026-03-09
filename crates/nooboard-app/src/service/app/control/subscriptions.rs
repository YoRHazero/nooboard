use crate::AppResult;
use crate::service::types::{EventSubscription, StateSubscription};

use super::state::ControlState;

pub(super) fn subscribe_state(state: &ControlState) -> AppResult<StateSubscription> {
    Ok(state.state_hub.subscribe())
}

pub(super) fn subscribe_events(state: &ControlState) -> AppResult<EventSubscription> {
    Ok(state.event_hub.subscribe())
}
