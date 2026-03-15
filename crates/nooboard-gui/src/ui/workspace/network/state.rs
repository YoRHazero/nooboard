use std::collections::BTreeSet;

use gpui::{AppContext as _, Context, Entity, Window};
use gpui_component::input::InputState;
use nooboard_core::{DirectRequestId, DirectSeedId, DirectSeedInfo, SessionId};

use crate::{
    ui::workspace::WorkspaceView,
    workspace::view_state::{NetworkDirectSeedViewState, NetworkPageViewState},
};

#[derive(Clone)]
pub struct NetworkSearchResult {
    pub id: DirectSeedId,
    pub label: String,
    pub host: String,
    pub port: u16,
    pub endpoint_label: String,
    pub enabled: bool,
    pub learned_device_id: Option<String>,
}

pub(in crate::ui::workspace) struct NetworkPageState {
    seed_label_input: Entity<InputState>,
    seed_host_input: Entity<InputState>,
    seed_port_input: Entity<InputState>,
    search_input: Entity<InputState>,
    draft_enabled: bool,
    editing_seed_id: Option<DirectSeedId>,
    search_results: Vec<NetworkSearchResult>,
    pending_seed_ids: BTreeSet<DirectSeedId>,
    pending_request_ids: BTreeSet<DirectRequestId>,
    pending_session_ids: BTreeSet<SessionId>,
    search_in_flight: bool,
    save_in_flight: bool,
    feedback: Option<String>,
}

impl NetworkPageState {
    pub(in crate::ui::workspace) fn new(
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) -> Self {
        Self {
            seed_label_input: cx.new(|cx| InputState::new(window, cx).placeholder("Office relay")),
            seed_host_input: cx
                .new(|cx| InputState::new(window, cx).placeholder("relay.example.com")),
            seed_port_input: cx.new(|cx| InputState::new(window, cx).placeholder("17890")),
            search_input: cx.new(|cx| {
                InputState::new(window, cx).placeholder("Search direct seeds by label or host")
            }),
            draft_enabled: true,
            editing_seed_id: None,
            search_results: Vec::new(),
            pending_seed_ids: BTreeSet::new(),
            pending_request_ids: BTreeSet::new(),
            pending_session_ids: BTreeSet::new(),
            search_in_flight: false,
            save_in_flight: false,
            feedback: None,
        }
    }

    pub(in crate::ui::workspace) fn input_entities(&self) -> Vec<Entity<InputState>> {
        vec![
            self.seed_label_input.clone(),
            self.seed_host_input.clone(),
            self.seed_port_input.clone(),
            self.search_input.clone(),
        ]
    }

    pub(in crate::ui::workspace) fn seed_label_input(&self) -> Entity<InputState> {
        self.seed_label_input.clone()
    }

    pub(in crate::ui::workspace) fn seed_host_input(&self) -> Entity<InputState> {
        self.seed_host_input.clone()
    }

    pub(in crate::ui::workspace) fn seed_port_input(&self) -> Entity<InputState> {
        self.seed_port_input.clone()
    }

    pub(in crate::ui::workspace) fn search_input(&self) -> Entity<InputState> {
        self.search_input.clone()
    }

    pub(in crate::ui::workspace) fn draft_enabled(&self) -> bool {
        self.draft_enabled
    }

    pub(in crate::ui::workspace) fn editing_seed_id(&self) -> Option<DirectSeedId> {
        self.editing_seed_id
    }

    pub(in crate::ui::workspace) fn search_results(&self) -> &[NetworkSearchResult] {
        &self.search_results
    }

    pub(in crate::ui::workspace) fn feedback(&self) -> Option<&String> {
        self.feedback.as_ref()
    }

    pub(in crate::ui::workspace) fn search_in_flight(&self) -> bool {
        self.search_in_flight
    }

    pub(in crate::ui::workspace) fn save_in_flight(&self) -> bool {
        self.save_in_flight
    }

    pub(in crate::ui::workspace) fn seed_pending(&self, id: DirectSeedId) -> bool {
        self.pending_seed_ids.contains(&id)
    }

    pub(in crate::ui::workspace) fn request_pending(&self, id: DirectRequestId) -> bool {
        self.pending_request_ids.contains(&id)
    }

    pub(in crate::ui::workspace) fn session_pending(&self, id: SessionId) -> bool {
        self.pending_session_ids.contains(&id)
    }

    pub(in crate::ui::workspace) fn seed_label(&self, cx: &Context<WorkspaceView>) -> String {
        self.seed_label_input.read(cx).value().to_string()
    }

    pub(in crate::ui::workspace) fn seed_host(&self, cx: &Context<WorkspaceView>) -> String {
        self.seed_host_input.read(cx).value().to_string()
    }

    pub(in crate::ui::workspace) fn seed_port(&self, cx: &Context<WorkspaceView>) -> String {
        self.seed_port_input.read(cx).value().to_string()
    }

    pub(in crate::ui::workspace) fn search_query(&self, cx: &Context<WorkspaceView>) -> String {
        self.search_input.read(cx).value().to_string()
    }

    pub(in crate::ui::workspace) fn sync_from_workspace(
        &mut self,
        page: Option<&NetworkPageViewState>,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        let Some(page) = page else {
            return;
        };

        if let Some(editing_seed_id) = self.editing_seed_id
            && !page
                .direct_seeds
                .iter()
                .any(|seed| seed.id == editing_seed_id)
        {
            self.clear_seed_draft(window, cx);
        }
        self.pending_seed_ids
            .retain(|id| page.direct_seeds.iter().any(|seed| seed.id == *id));
        self.pending_request_ids.retain(|id| {
            page.pending_requests
                .iter()
                .any(|request| request.id == *id)
        });
        self.pending_session_ids
            .retain(|id| page.sessions.iter().any(|session| session.id == *id));
    }

    pub(in crate::ui::workspace) fn clear_seed_draft(
        &mut self,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        self.editing_seed_id = None;
        self.draft_enabled = true;
        self.seed_label_input.update(cx, |input, cx| {
            input.set_value(String::new(), window, cx);
        });
        self.seed_host_input.update(cx, |input, cx| {
            input.set_value(String::new(), window, cx);
        });
        self.seed_port_input.update(cx, |input, cx| {
            input.set_value(String::new(), window, cx);
        });
    }

    pub(in crate::ui::workspace) fn load_seed_into_draft(
        &mut self,
        seed: &NetworkDirectSeedViewState,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        self.editing_seed_id = Some(seed.id);
        self.draft_enabled = seed.enabled;
        self.seed_label_input.update(cx, |input, cx| {
            input.set_value(seed.label.clone(), window, cx);
        });
        self.seed_host_input.update(cx, |input, cx| {
            input.set_value(seed.host.clone(), window, cx);
        });
        self.seed_port_input.update(cx, |input, cx| {
            input.set_value(seed.port.to_string(), window, cx);
        });
        self.feedback = Some(format!(
            "Loaded direct seed '{}' into the composer.",
            seed.label
        ));
    }

    pub(in crate::ui::workspace) fn toggle_draft_enabled(&mut self) {
        self.draft_enabled = !self.draft_enabled;
    }

    pub(in crate::ui::workspace) fn begin_search(&mut self) {
        self.search_in_flight = true;
        self.feedback = Some("Searching configured direct seeds.".to_string());
    }

    pub(in crate::ui::workspace) fn finish_search(&mut self, results: Vec<DirectSeedInfo>) {
        self.search_in_flight = false;
        self.search_results = results
            .into_iter()
            .map(|seed| NetworkSearchResult {
                id: seed.id,
                label: seed.label,
                host: seed.host.clone(),
                port: seed.port,
                endpoint_label: format!("{}:{}", seed.host, seed.port),
                enabled: seed.enabled,
                learned_device_id: seed.learned_device_id,
            })
            .collect();
        self.feedback = Some(format!(
            "Found {} direct seed result(s).",
            self.search_results.len()
        ));
    }

    pub(in crate::ui::workspace) fn fail_search(&mut self, message: String) {
        self.search_in_flight = false;
        self.feedback = Some(message);
    }

    pub(in crate::ui::workspace) fn begin_save(&mut self) {
        self.save_in_flight = true;
        self.feedback = Some("Saving direct seed configuration.".to_string());
    }

    pub(in crate::ui::workspace) fn finish_save(&mut self, saved_id: DirectSeedId) {
        self.save_in_flight = false;
        self.editing_seed_id = Some(saved_id);
        self.feedback = Some("Direct seed saved.".to_string());
    }

    pub(in crate::ui::workspace) fn fail_save(&mut self, message: String) {
        self.save_in_flight = false;
        self.feedback = Some(message);
    }

    pub(in crate::ui::workspace) fn mark_seed_pending(
        &mut self,
        id: DirectSeedId,
        message: String,
    ) {
        self.pending_seed_ids.insert(id);
        self.feedback = Some(message);
    }

    pub(in crate::ui::workspace) fn finish_seed_pending(
        &mut self,
        id: DirectSeedId,
        message: String,
    ) {
        self.pending_seed_ids.remove(&id);
        self.feedback = Some(message);
    }

    pub(in crate::ui::workspace) fn mark_request_pending(
        &mut self,
        id: DirectRequestId,
        message: String,
    ) {
        self.pending_request_ids.insert(id);
        self.feedback = Some(message);
    }

    pub(in crate::ui::workspace) fn finish_request_pending(
        &mut self,
        id: DirectRequestId,
        message: String,
    ) {
        self.pending_request_ids.remove(&id);
        self.feedback = Some(message);
    }

    pub(in crate::ui::workspace) fn mark_session_pending(
        &mut self,
        id: SessionId,
        message: String,
    ) {
        self.pending_session_ids.insert(id);
        self.feedback = Some(message);
    }

    pub(in crate::ui::workspace) fn finish_session_pending(
        &mut self,
        id: SessionId,
        message: String,
    ) {
        self.pending_session_ids.remove(&id);
        self.feedback = Some(message);
    }

    pub(in crate::ui::workspace) fn set_feedback(&mut self, message: impl Into<String>) {
        self.feedback = Some(message.into());
    }
}
