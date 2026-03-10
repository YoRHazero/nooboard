use gpui::{AppContext, Context, Entity, Window};
use gpui_component::input::InputState;

use super::WorkspaceView;
use super::snapshot::{
    ClipboardSettingsValue, NetworkSettingsValue, SettingsSnapshot, StorageSettingsValue,
    TransferSettingsValue,
};
use super::{SettingsSection, SettingsSectionPhase};

pub(in crate::ui::workspace::view) struct SettingsPageState {
    pub(super) network: SettingsSection<NetworkSettingsValue>,
    pub(super) storage: SettingsSection<StorageSettingsValue>,
    pub(super) clipboard: SettingsSection<ClipboardSettingsValue>,
    pub(super) transfers: SettingsSection<TransferSettingsValue>,
    pub(super) manual_peer_input: Entity<InputState>,
    pub(super) feedback: Option<String>,
}

impl SettingsPageState {
    pub(in crate::ui::workspace::view) fn new(
        snapshot: SettingsSnapshot,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) -> Self {
        let manual_peer_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("127.0.0.1:24001"));

        Self {
            network: SettingsSection::new(snapshot.network),
            storage: SettingsSection::new(snapshot.storage),
            clipboard: SettingsSection::new(snapshot.clipboard),
            transfers: SettingsSection::new(snapshot.transfers),
            manual_peer_input,
            feedback: None,
        }
    }

    pub(super) fn sync_from_snapshot(&mut self, snapshot: SettingsSnapshot) {
        self.network.sync_from_live(snapshot.network);
        self.storage.sync_from_live(snapshot.storage);
        self.clipboard.sync_from_live(snapshot.clipboard);
        self.transfers.sync_from_live(snapshot.transfers);
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
