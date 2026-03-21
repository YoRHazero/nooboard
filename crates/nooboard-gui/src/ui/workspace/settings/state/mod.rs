mod clipboard;
mod connection;
mod storage;
mod transfers;

use gpui::{Context, Entity, Window};
use gpui_component::input::InputState;

use crate::{
    ui::workspace::WorkspaceView,
    workspace::view_state::{
        SettingsClipboardViewState, SettingsConnectionViewState, SettingsPageViewState,
        SettingsStorageViewState, SettingsTransfersViewState,
    },
};

use self::{
    clipboard::ClipboardSettingsState, connection::ConnectionSettingsState,
    storage::StorageSettingsState, transfers::TransferSettingsState,
};

pub(in crate::ui::workspace) use self::storage::{StorageBytesUnit, StorageDurationUnit};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SettingsSectionKey {
    Connection,
    Clipboard,
    Transfers,
    Storage,
}

pub(in crate::ui::workspace) struct SettingsPageState {
    connection: ConnectionSettingsState,
    clipboard: ClipboardSettingsState,
    transfers: TransferSettingsState,
    storage: StorageSettingsState,
    applying: Option<SettingsSectionKey>,
    feedback: Option<String>,
}

impl SettingsPageState {
    pub(in crate::ui::workspace) fn new(
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) -> Self {
        Self {
            connection: ConnectionSettingsState::new(window, cx),
            clipboard: ClipboardSettingsState::new(),
            transfers: TransferSettingsState::new(window, cx),
            storage: StorageSettingsState::new(window, cx),
            applying: None,
            feedback: None,
        }
    }

    pub(in crate::ui::workspace) fn input_entities(&self) -> Vec<Entity<InputState>> {
        self.connection
            .input_entities()
            .into_iter()
            .chain(self.transfers.input_entities())
            .chain(self.storage.input_entities())
            .collect()
    }

    pub(in crate::ui::workspace) fn sync_from_workspace(
        &mut self,
        page: Option<&SettingsPageViewState>,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        let Some(page) = page else {
            return;
        };

        self.connection
            .sync_from_workspace(&page.connection, window, cx);
        self.clipboard.sync_from_workspace(&page.clipboard);
        self.transfers
            .sync_from_workspace(&page.transfers, window, cx);
        self.storage.sync_from_workspace(&page.storage, window, cx);
    }

    pub(in crate::ui::workspace) fn device_id_input(&self) -> Entity<InputState> {
        self.connection.device_id_input()
    }

    pub(in crate::ui::workspace) fn token_input(&self) -> Entity<InputState> {
        self.connection.token_input()
    }

    pub(in crate::ui::workspace) fn listen_port_input(&self) -> Entity<InputState> {
        self.connection.listen_port_input()
    }

    pub(in crate::ui::workspace) fn download_dir_input(&self) -> Entity<InputState> {
        self.transfers.download_dir_input()
    }

    pub(in crate::ui::workspace) fn history_window_input(&self) -> Entity<InputState> {
        self.storage.history_window_input()
    }

    pub(in crate::ui::workspace) fn dedup_window_input(&self) -> Entity<InputState> {
        self.storage.dedup_window_input()
    }

    pub(in crate::ui::workspace) fn max_text_bytes_input(&self) -> Entity<InputState> {
        self.storage.max_text_bytes_input()
    }

    pub(in crate::ui::workspace) fn lan_enabled(&self) -> bool {
        self.connection.lan_enabled()
    }

    pub(in crate::ui::workspace) fn token_masked(&self) -> bool {
        self.connection.token_masked()
    }

    pub(in crate::ui::workspace) fn local_capture_enabled(&self) -> bool {
        self.clipboard.local_capture_enabled()
    }

    pub(in crate::ui::workspace) fn applying(&self) -> Option<SettingsSectionKey> {
        self.applying
    }

    pub(in crate::ui::workspace) fn feedback(&self) -> Option<&String> {
        self.feedback.as_ref()
    }

    pub(in crate::ui::workspace) fn toggle_lan_enabled(&mut self) {
        self.connection.toggle_lan_enabled();
    }

    pub(in crate::ui::workspace) fn toggle_token_masked(
        &mut self,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        self.connection.toggle_token_masked(window, cx);
    }

    pub(in crate::ui::workspace) fn toggle_local_capture_enabled(&mut self) {
        self.clipboard.toggle_local_capture_enabled();
    }

    pub(in crate::ui::workspace) fn reset_connection_from_workspace(
        &mut self,
        page: &SettingsConnectionViewState,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        self.connection.reset_from_workspace(page, window, cx);
    }

    pub(in crate::ui::workspace) fn reset_clipboard_from_workspace(
        &mut self,
        page: &SettingsClipboardViewState,
    ) {
        self.clipboard.reset_from_workspace(page);
    }

    pub(in crate::ui::workspace) fn reset_transfer_from_workspace(
        &mut self,
        page: &SettingsTransfersViewState,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        self.transfers.reset_from_workspace(page, window, cx);
    }

    pub(in crate::ui::workspace) fn reset_storage_from_workspace(
        &mut self,
        page: &SettingsStorageViewState,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        self.storage.reset_from_workspace(page, window, cx);
    }

    pub(in crate::ui::workspace) fn begin_apply(
        &mut self,
        section: SettingsSectionKey,
        message: String,
    ) {
        self.applying = Some(section);
        self.feedback = Some(message);
    }

    pub(in crate::ui::workspace) fn finish_apply(&mut self, message: String) {
        self.applying = None;
        self.feedback = Some(message);
    }

    pub(in crate::ui::workspace) fn fail_apply(&mut self, message: String) {
        self.applying = None;
        self.feedback = Some(message);
    }

    pub(in crate::ui::workspace) fn set_feedback(&mut self, message: String) {
        self.feedback = Some(message);
    }

    pub(in crate::ui::workspace) fn set_download_dir_value(
        &mut self,
        value: String,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        self.transfers.set_download_dir_value(value, window, cx);
    }

    pub(in crate::ui::workspace) fn device_id_value(&self, cx: &Context<WorkspaceView>) -> String {
        self.connection.device_id_value(cx)
    }

    pub(in crate::ui::workspace) fn token_value(&self, cx: &Context<WorkspaceView>) -> String {
        self.connection.token_value(cx)
    }

    pub(in crate::ui::workspace) fn listen_port_value(
        &self,
        cx: &Context<WorkspaceView>,
    ) -> String {
        self.connection.listen_port_value(cx)
    }

    pub(in crate::ui::workspace) fn download_dir_value(
        &self,
        cx: &Context<WorkspaceView>,
    ) -> String {
        self.transfers.download_dir_value(cx)
    }

    pub(in crate::ui::workspace) fn history_window_unit(&self) -> StorageDurationUnit {
        self.storage.history_window_unit()
    }

    pub(in crate::ui::workspace) fn dedup_window_unit(&self) -> StorageDurationUnit {
        self.storage.dedup_window_unit()
    }

    pub(in crate::ui::workspace) fn max_text_bytes_unit(&self) -> StorageBytesUnit {
        self.storage.max_text_bytes_unit()
    }

    pub(in crate::ui::workspace) fn select_history_window_unit(
        &mut self,
        unit: StorageDurationUnit,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        self.storage.select_history_window_unit(unit, window, cx);
    }

    pub(in crate::ui::workspace) fn select_dedup_window_unit(
        &mut self,
        unit: StorageDurationUnit,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        self.storage.select_dedup_window_unit(unit, window, cx);
    }

    pub(in crate::ui::workspace) fn select_max_text_bytes_unit(
        &mut self,
        unit: StorageBytesUnit,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        self.storage.select_max_text_bytes_unit(unit, window, cx);
    }

    pub(in crate::ui::workspace) fn history_window_days(
        &self,
        cx: &Context<WorkspaceView>,
    ) -> Result<u32, String> {
        self.storage.history_window_days(cx)
    }

    pub(in crate::ui::workspace) fn dedup_window_days(
        &self,
        cx: &Context<WorkspaceView>,
    ) -> Result<u32, String> {
        self.storage.dedup_window_days(cx)
    }

    pub(in crate::ui::workspace) fn max_text_bytes(
        &self,
        cx: &Context<WorkspaceView>,
    ) -> Result<usize, String> {
        self.storage.max_text_bytes(cx)
    }
}
