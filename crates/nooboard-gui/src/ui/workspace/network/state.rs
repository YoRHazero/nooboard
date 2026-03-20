use std::collections::BTreeSet;

use gpui::{AppContext as _, Context, Entity, Window};
use gpui_component::input::InputState;
use nooboard_core::{DirectRequestId, DirectSeedId, SessionId};

use crate::{
    ui::workspace::WorkspaceView,
    workspace::view_state::{NetworkDirectSeedViewState, NetworkPageViewState},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DirectPanelTab {
    Seeds,
    Pending,
    Sessions,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SeedPanelMode {
    Create,
    Search,
}

pub(in crate::ui::workspace) struct NetworkPageState {
    seed_label_input: Entity<InputState>,
    seed_host_input: Entity<InputState>,
    seed_port_input: Entity<InputState>,
    seed_filter_input: Entity<InputState>,
    direct_tab: DirectPanelTab,
    seed_panel_mode: SeedPanelMode,
    draft_enabled: bool,
    editing_seed_id: Option<DirectSeedId>,
    token_revealed: bool,
    pending_seed_ids: BTreeSet<DirectSeedId>,
    pending_request_ids: BTreeSet<DirectRequestId>,
    pending_session_ids: BTreeSet<SessionId>,
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
            seed_filter_input: cx.new(|cx| {
                InputState::new(window, cx).placeholder("Filter by label or learned device id")
            }),
            direct_tab: DirectPanelTab::Seeds,
            seed_panel_mode: SeedPanelMode::Create,
            draft_enabled: true,
            editing_seed_id: None,
            token_revealed: false,
            pending_seed_ids: BTreeSet::new(),
            pending_request_ids: BTreeSet::new(),
            pending_session_ids: BTreeSet::new(),
            save_in_flight: false,
            feedback: None,
        }
    }

    pub(in crate::ui::workspace) fn input_entities(&self) -> Vec<Entity<InputState>> {
        vec![
            self.seed_label_input.clone(),
            self.seed_host_input.clone(),
            self.seed_port_input.clone(),
            self.seed_filter_input.clone(),
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

    pub(in crate::ui::workspace) fn seed_filter_input(&self) -> Entity<InputState> {
        self.seed_filter_input.clone()
    }

    pub(in crate::ui::workspace) fn direct_tab(&self) -> DirectPanelTab {
        self.direct_tab
    }

    pub(in crate::ui::workspace) fn set_direct_tab(&mut self, tab: DirectPanelTab) {
        self.direct_tab = tab;
    }

    pub(in crate::ui::workspace) fn seed_panel_mode(&self) -> SeedPanelMode {
        self.seed_panel_mode
    }

    pub(in crate::ui::workspace) fn set_seed_panel_mode(&mut self, mode: SeedPanelMode) {
        self.seed_panel_mode = mode;
    }

    pub(in crate::ui::workspace) fn draft_enabled(&self) -> bool {
        self.draft_enabled
    }

    pub(in crate::ui::workspace) fn editing_seed_id(&self) -> Option<DirectSeedId> {
        self.editing_seed_id
    }

    pub(in crate::ui::workspace) fn token_revealed(&self) -> bool {
        self.token_revealed
    }

    pub(in crate::ui::workspace) fn toggle_token_revealed(&mut self) {
        self.token_revealed = !self.token_revealed;
    }

    pub(in crate::ui::workspace) fn feedback(&self) -> Option<&String> {
        self.feedback.as_ref()
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

    pub(in crate::ui::workspace) fn seed_filter(&self, cx: &Context<WorkspaceView>) -> String {
        self.seed_filter_input.read(cx).value().to_string()
    }

    pub(in crate::ui::workspace) fn filtered_direct_seeds(
        &self,
        page: &NetworkPageViewState,
        cx: &Context<WorkspaceView>,
    ) -> Vec<NetworkDirectSeedViewState> {
        let query = self.seed_filter(cx).trim().to_lowercase();
        if query.is_empty() {
            return page.direct_seeds.clone();
        }

        page.direct_seeds
            .iter()
            .filter(|seed| {
                seed.label.to_lowercase().contains(&query)
                    || seed
                        .learned_device_id
                        .as_ref()
                        .is_some_and(|device_id| device_id.to_lowercase().contains(&query))
            })
            .cloned()
            .collect()
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
            && !page.direct_seeds.iter().any(|seed| seed.id == editing_seed_id)
        {
            self.clear_seed_draft(window, cx);
        }
        self.pending_seed_ids
            .retain(|id| page.direct_seeds.iter().any(|seed| seed.id == *id));
        self.pending_request_ids
            .retain(|id| page.pending_requests.iter().any(|request| request.id == *id));
        self.pending_session_ids
            .retain(|id| page.direct_sessions.iter().any(|session| session.id == *id));
    }

    pub(in crate::ui::workspace) fn clear_seed_draft(
        &mut self,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        self.editing_seed_id = None;
        self.seed_panel_mode = SeedPanelMode::Create;
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
        self.direct_tab = DirectPanelTab::Seeds;
        self.seed_panel_mode = SeedPanelMode::Create;
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
        self.feedback = Some(format!("Loaded direct seed '{}' into the editor.", seed.label));
    }

    pub(in crate::ui::workspace) fn toggle_draft_enabled(&mut self) {
        self.draft_enabled = !self.draft_enabled;
    }

    pub(in crate::ui::workspace) fn begin_save(&mut self) {
        self.save_in_flight = true;
        self.feedback = Some("Saving direct preset.".to_string());
    }

    pub(in crate::ui::workspace) fn finish_save(&mut self, saved_id: DirectSeedId) {
        self.save_in_flight = false;
        self.editing_seed_id = Some(saved_id);
        self.feedback = Some("Direct preset saved.".to_string());
    }

    pub(in crate::ui::workspace) fn fail_save(&mut self, message: String) {
        self.save_in_flight = false;
        self.feedback = Some(message);
    }

    pub(in crate::ui::workspace) fn mark_seed_pending(&mut self, id: DirectSeedId, message: String) {
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
