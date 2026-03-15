use std::collections::BTreeSet;

use nooboard_core::{EventId, SessionId, SessionTarget};

use crate::workspace::view_state::WorkspaceSessionTargetViewState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::ui::workspace) enum ClipboardBroadcastScope {
    AllConnected,
    SelectedSessions,
}

pub(super) struct ClipboardTargetSelectionState {
    broadcast_scope: ClipboardBroadcastScope,
    selected_session_ids: BTreeSet<SessionId>,
    adopt_in_flight_event_id: Option<EventId>,
    rebroadcast_in_flight_event_id: Option<EventId>,
}

impl ClipboardTargetSelectionState {
    pub(super) fn new() -> Self {
        Self {
            broadcast_scope: ClipboardBroadcastScope::AllConnected,
            selected_session_ids: BTreeSet::new(),
            adopt_in_flight_event_id: None,
            rebroadcast_in_flight_event_id: None,
        }
    }

    pub(super) fn broadcast_scope(&self) -> ClipboardBroadcastScope {
        self.broadcast_scope
    }

    pub(super) fn selected_session_ids(&self) -> &BTreeSet<SessionId> {
        &self.selected_session_ids
    }

    pub(super) fn adopt_in_flight_event_id(&self) -> Option<EventId> {
        self.adopt_in_flight_event_id
    }

    pub(super) fn rebroadcast_in_flight_event_id(&self) -> Option<EventId> {
        self.rebroadcast_in_flight_event_id
    }

    pub(super) fn set_broadcast_scope(&mut self, scope: ClipboardBroadcastScope) {
        self.broadcast_scope = scope;
    }

    pub(super) fn toggle_session_target(&mut self, session_id: SessionId) {
        if self.broadcast_scope != ClipboardBroadcastScope::SelectedSessions {
            return;
        }

        if self.selected_session_ids.contains(&session_id) {
            self.selected_session_ids.remove(&session_id);
        } else {
            self.selected_session_ids.insert(session_id);
        }
    }

    pub(super) fn rebroadcast_target(&self) -> SessionTarget {
        match self.broadcast_scope {
            ClipboardBroadcastScope::AllConnected => SessionTarget::AllConnected,
            ClipboardBroadcastScope::SelectedSessions => {
                SessionTarget::Sessions(self.selected_session_ids.iter().cloned().collect())
            }
        }
    }

    pub(super) fn retain_connected_sessions(
        &mut self,
        sessions: &[WorkspaceSessionTargetViewState],
    ) {
        let connected = sessions
            .iter()
            .map(|target| target.id)
            .collect::<std::collections::HashSet<_>>();
        self.selected_session_ids
            .retain(|session_id| connected.contains(session_id));
    }

    pub(super) fn start_adopt(&mut self, event_id: EventId) {
        self.adopt_in_flight_event_id = Some(event_id);
    }

    pub(super) fn finish_adopt(&mut self, event_id: EventId) {
        if self.adopt_in_flight_event_id == Some(event_id) {
            self.adopt_in_flight_event_id = None;
        }
    }

    pub(super) fn start_rebroadcast(&mut self, event_id: EventId) {
        self.rebroadcast_in_flight_event_id = Some(event_id);
    }

    pub(super) fn finish_rebroadcast(&mut self, event_id: EventId) {
        if self.rebroadcast_in_flight_event_id == Some(event_id) {
            self.rebroadcast_in_flight_event_id = None;
        }
    }
}
