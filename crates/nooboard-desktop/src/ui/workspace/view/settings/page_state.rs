use gpui::{AppContext, Context, Entity, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};

use super::WorkspaceView;
use super::snapshot::{
    ClipboardSettingsValue, LocalConnectionInfoValue, NetworkPanelValue, SettingsSnapshot,
    StorageSettingsValue, TransferSettingsValue,
};
use super::{SettingsSection, SettingsSectionPhase};

pub(in crate::ui::workspace::view) struct SettingsPageState {
    pub(super) network: SettingsSection<NetworkPanelValue>,
    pub(super) local_connection: LocalConnectionInfoValue,
    pub(super) storage: SettingsSection<StorageSettingsValue>,
    pub(super) clipboard: SettingsSection<ClipboardSettingsValue>,
    pub(super) transfers: SettingsSection<TransferSettingsValue>,
    pub(super) manual_peer_input: Entity<InputState>,
    pub(super) device_id_input: Entity<InputState>,
    pub(super) token_input: Entity<InputState>,
    pub(super) listen_port_input: Entity<InputState>,
    pub(super) token_visible: bool,
    pub(super) feedback: Option<String>,
    syncing_network_inputs: bool,
    _subscriptions: Vec<Subscription>,
}

impl SettingsPageState {
    pub(in crate::ui::workspace::view) fn new(
        snapshot: SettingsSnapshot,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) -> Self {
        let network_panel = NetworkPanelValue::from_snapshot(&snapshot);
        let manual_peer_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("127.0.0.1:24001"));
        let device_id_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(network_panel.device_id.clone())
                .placeholder("My laptop")
        });
        let token_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(network_panel.token.clone())
                .placeholder("shared-sync-token")
                .masked(true)
        });
        let listen_port_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(network_panel.listen_port_text.clone())
                .placeholder("17890")
                .validate(|value, _cx| {
                    value.is_empty()
                        || (value.chars().all(|ch| ch.is_ascii_digit()) && value.len() <= 5)
                })
        });

        let mut page_state = Self {
            network: SettingsSection::new(network_panel),
            local_connection: snapshot.local_connection,
            storage: SettingsSection::new(snapshot.storage),
            clipboard: SettingsSection::new(snapshot.clipboard),
            transfers: SettingsSection::new(snapshot.transfers),
            manual_peer_input,
            device_id_input,
            token_input,
            listen_port_input,
            token_visible: false,
            feedback: None,
            syncing_network_inputs: false,
            _subscriptions: Vec::new(),
        };
        page_state._subscriptions = page_state.build_subscriptions(cx);
        page_state
    }

    pub(super) fn sync_from_snapshot(&mut self, snapshot: SettingsSnapshot) {
        self.network
            .sync_from_live(NetworkPanelValue::from_snapshot(&snapshot));
        self.local_connection = snapshot.local_connection;
        self.storage.sync_from_live(snapshot.storage);
        self.clipboard.sync_from_live(snapshot.clipboard);
        self.transfers.sync_from_live(snapshot.transfers);
    }

    pub(super) fn sync_network_inputs(
        &mut self,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        let next_device_id = self.network.draft.device_id.clone();
        let next_token = self.network.draft.token.clone();
        let next_listen_port = self.network.draft.listen_port_text.clone();
        let current_device_id = self.device_id_input.read(cx).value().to_string();
        let current_token = self.token_input.read(cx).value().to_string();
        let current_listen_port = self.listen_port_input.read(cx).value().to_string();

        if current_device_id == next_device_id
            && current_token == next_token
            && current_listen_port == next_listen_port
        {
            return;
        }

        self.syncing_network_inputs = true;
        if current_device_id != next_device_id {
            let next_value = next_device_id.clone();
            let _ = self.device_id_input.update(cx, |input, cx| {
                input.set_value(next_value, window, cx);
            });
        }
        if current_token != next_token {
            let next_value = next_token.clone();
            let _ = self.token_input.update(cx, |input, cx| {
                input.set_value(next_value, window, cx);
            });
        }
        if current_listen_port != next_listen_port {
            let next_value = next_listen_port.clone();
            let _ = self.listen_port_input.update(cx, |input, cx| {
                input.set_value(next_value, window, cx);
            });
        }
        self.syncing_network_inputs = false;
    }

    fn build_subscriptions(&self, cx: &mut Context<WorkspaceView>) -> Vec<Subscription> {
        let device_id_input = self.device_id_input.clone();
        let token_input = self.token_input.clone();
        let listen_port_input = self.listen_port_input.clone();

        vec![
            cx.subscribe(&device_id_input, |this, input, event: &InputEvent, cx| {
                if !matches!(event, InputEvent::Change)
                    || this.settings_page_state.syncing_network_inputs
                {
                    return;
                }

                let next_value = input.read(cx).value().to_string();
                if this.settings_page_state.network.draft.device_id == next_value {
                    return;
                }

                this.settings_page_state.network.draft.device_id = next_value;
                this.settings_page_state.network.mark_edited();
                this.clear_settings_feedback();
                cx.notify();
            }),
            cx.subscribe(&token_input, |this, input, event: &InputEvent, cx| {
                if !matches!(event, InputEvent::Change)
                    || this.settings_page_state.syncing_network_inputs
                {
                    return;
                }

                let next_value = input.read(cx).value().to_string();
                if this.settings_page_state.network.draft.token == next_value {
                    return;
                }

                this.settings_page_state.network.draft.token = next_value;
                this.settings_page_state.network.mark_edited();
                this.clear_settings_feedback();
                cx.notify();
            }),
            cx.subscribe(&listen_port_input, |this, input, event: &InputEvent, cx| {
                if !matches!(event, InputEvent::Change)
                    || this.settings_page_state.syncing_network_inputs
                {
                    return;
                }

                let next_value = input.read(cx).value().to_string();
                if this.settings_page_state.network.draft.listen_port_text == next_value {
                    return;
                }

                this.settings_page_state.network.draft.listen_port_text = next_value;
                this.settings_page_state.network.mark_edited();
                this.clear_settings_feedback();
                cx.notify();
            }),
        ]
    }
}

impl WorkspaceView {
    pub(in crate::ui::workspace::view) fn settings_phase_summary(
        phase: &SettingsSectionPhase,
        stale_message: &'static str,
    ) -> Option<String> {
        if matches!(phase, SettingsSectionPhase::Stale) {
            Some(stale_message.to_string())
        } else if let SettingsSectionPhase::Error(message) = phase {
            Some(message.clone())
        } else {
            None
        }
    }
}
