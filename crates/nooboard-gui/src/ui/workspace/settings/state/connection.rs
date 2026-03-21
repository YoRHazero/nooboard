use gpui::{AppContext as _, Context, Entity, Window};
use gpui_component::input::InputState;

use crate::{ui::workspace::WorkspaceView, workspace::view_state::SettingsConnectionViewState};

#[derive(Clone)]
struct ConnectionSyncedValues {
    device_id: String,
    token: String,
    listen_port: String,
    lan_enabled: bool,
}

pub(super) struct ConnectionSettingsState {
    device_id_input: Entity<InputState>,
    token_input: Entity<InputState>,
    listen_port_input: Entity<InputState>,
    lan_enabled: bool,
    token_masked: bool,
    synced: Option<ConnectionSyncedValues>,
}

impl ConnectionSettingsState {
    pub(super) fn new(window: &mut Window, cx: &mut Context<WorkspaceView>) -> Self {
        Self {
            device_id_input: cx.new(|cx| InputState::new(window, cx).placeholder("My laptop")),
            token_input: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("shared-sync-token")
                    .masked(true)
            }),
            listen_port_input: cx.new(|cx| InputState::new(window, cx).placeholder("17890")),
            lan_enabled: true,
            token_masked: true,
            synced: None,
        }
    }

    pub(super) fn input_entities(&self) -> Vec<Entity<InputState>> {
        vec![
            self.device_id_input.clone(),
            self.token_input.clone(),
            self.listen_port_input.clone(),
        ]
    }

    pub(super) fn sync_from_workspace(
        &mut self,
        page: &SettingsConnectionViewState,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        let next = ConnectionSyncedValues {
            device_id: page.device_id.clone(),
            token: page.token.clone(),
            listen_port: page.listen_port.to_string(),
            lan_enabled: page.lan_enabled,
        };

        if let Some(previous) = self.synced.clone() {
            self.sync_input_if_clean(
                &self.device_id_input,
                &previous.device_id,
                &next.device_id,
                window,
                cx,
            );
            self.sync_input_if_clean(&self.token_input, &previous.token, &next.token, window, cx);
            self.sync_input_if_clean(
                &self.listen_port_input,
                &previous.listen_port,
                &next.listen_port,
                window,
                cx,
            );
            if self.lan_enabled == previous.lan_enabled {
                self.lan_enabled = next.lan_enabled;
            }
        } else {
            self.force_sync_all(&next, window, cx);
            self.lan_enabled = next.lan_enabled;
        }

        self.synced = Some(next);
    }

    pub(super) fn device_id_input(&self) -> Entity<InputState> {
        self.device_id_input.clone()
    }

    pub(super) fn token_input(&self) -> Entity<InputState> {
        self.token_input.clone()
    }

    pub(super) fn listen_port_input(&self) -> Entity<InputState> {
        self.listen_port_input.clone()
    }

    pub(super) fn lan_enabled(&self) -> bool {
        self.lan_enabled
    }

    pub(super) fn token_masked(&self) -> bool {
        self.token_masked
    }

    pub(super) fn toggle_lan_enabled(&mut self) {
        self.lan_enabled = !self.lan_enabled;
    }

    pub(super) fn toggle_token_masked(
        &mut self,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        self.token_masked = !self.token_masked;
        let masked = self.token_masked;
        self.token_input.update(cx, |input, cx| {
            input.set_masked(masked, window, cx);
        });
    }

    pub(super) fn reset_from_workspace(
        &mut self,
        page: &SettingsConnectionViewState,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        let next = ConnectionSyncedValues {
            device_id: page.device_id.clone(),
            token: page.token.clone(),
            listen_port: page.listen_port.to_string(),
            lan_enabled: page.lan_enabled,
        };
        self.force_sync_all(&next, window, cx);
        self.lan_enabled = next.lan_enabled;
        self.token_masked = true;
        self.token_input.update(cx, |input, cx| {
            input.set_masked(true, window, cx);
        });
        self.synced = Some(next);
    }

    pub(super) fn device_id_value(&self, cx: &Context<WorkspaceView>) -> String {
        self.device_id_input.read(cx).value().to_string()
    }

    pub(super) fn token_value(&self, cx: &Context<WorkspaceView>) -> String {
        self.token_input.read(cx).value().to_string()
    }

    pub(super) fn listen_port_value(&self, cx: &Context<WorkspaceView>) -> String {
        self.listen_port_input.read(cx).value().to_string()
    }

    fn sync_input_if_clean(
        &self,
        entity: &Entity<InputState>,
        previous_value: &str,
        next_value: &str,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        let current = entity.read(cx).value().to_string();
        if current == previous_value {
            entity.update(cx, |input, cx| {
                input.set_value(next_value.to_string(), window, cx);
            });
        }
    }

    fn force_sync_all(
        &self,
        next: &ConnectionSyncedValues,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        for (entity, value) in [
            (&self.device_id_input, next.device_id.clone()),
            (&self.token_input, next.token.clone()),
            (&self.listen_port_input, next.listen_port.clone()),
        ] {
            entity.update(cx, |input, cx| {
                input.set_value(value.clone(), window, cx);
            });
        }
    }
}
