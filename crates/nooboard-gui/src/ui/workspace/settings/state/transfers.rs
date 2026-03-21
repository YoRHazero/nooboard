use gpui::{AppContext as _, Context, Entity, Window};
use gpui_component::input::InputState;

use crate::{ui::workspace::WorkspaceView, workspace::view_state::SettingsTransfersViewState};

pub(super) struct TransferSettingsState {
    download_dir_input: Entity<InputState>,
    synced_download_dir: Option<String>,
}

impl TransferSettingsState {
    pub(super) fn new(window: &mut Window, cx: &mut Context<WorkspaceView>) -> Self {
        Self {
            download_dir_input: cx
                .new(|cx| InputState::new(window, cx).placeholder("/tmp/nooboard-downloads")),
            synced_download_dir: None,
        }
    }

    pub(super) fn input_entities(&self) -> Vec<Entity<InputState>> {
        vec![self.download_dir_input.clone()]
    }

    pub(super) fn sync_from_workspace(
        &mut self,
        page: &SettingsTransfersViewState,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        let next = page.download_dir.display().to_string();
        if self
            .synced_download_dir
            .as_ref()
            .is_none_or(|previous| self.download_dir_value(cx) == *previous)
        {
            self.download_dir_input.update(cx, |input, cx| {
                input.set_value(next.clone(), window, cx);
            });
        }
        self.synced_download_dir = Some(next);
    }

    pub(super) fn download_dir_input(&self) -> Entity<InputState> {
        self.download_dir_input.clone()
    }

    pub(super) fn set_download_dir_value(
        &mut self,
        value: String,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        self.download_dir_input.update(cx, |input, cx| {
            input.set_value(value, window, cx);
        });
    }

    pub(super) fn download_dir_value(&self, cx: &Context<WorkspaceView>) -> String {
        self.download_dir_input.read(cx).value().to_string()
    }

    pub(super) fn reset_from_workspace(
        &mut self,
        page: &SettingsTransfersViewState,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        let next = page.download_dir.display().to_string();
        self.download_dir_input.update(cx, |input, cx| {
            input.set_value(next.clone(), window, cx);
        });
        self.synced_download_dir = Some(next);
    }
}
