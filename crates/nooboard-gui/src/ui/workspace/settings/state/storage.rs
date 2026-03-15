use gpui::{AppContext as _, Context, Entity, Window};
use gpui_component::input::InputState;

use crate::{ui::workspace::WorkspaceView, workspace::view_state::SettingsStorageViewState};

#[derive(Clone)]
struct StorageSyncedValues {
    history_window_days: String,
    dedup_window_days: String,
    max_text_bytes: String,
    gc_batch_size: String,
}

pub(super) struct StorageSettingsState {
    history_window_input: Entity<InputState>,
    dedup_window_input: Entity<InputState>,
    max_text_bytes_input: Entity<InputState>,
    gc_batch_size_input: Entity<InputState>,
    synced: Option<StorageSyncedValues>,
}

impl StorageSettingsState {
    pub(super) fn new(window: &mut Window, cx: &mut Context<WorkspaceView>) -> Self {
        Self {
            history_window_input: cx.new(|cx| InputState::new(window, cx).placeholder("7")),
            dedup_window_input: cx.new(|cx| InputState::new(window, cx).placeholder("14")),
            max_text_bytes_input: cx.new(|cx| InputState::new(window, cx).placeholder("4096")),
            gc_batch_size_input: cx.new(|cx| InputState::new(window, cx).placeholder("64")),
            synced: None,
        }
    }

    pub(super) fn input_entities(&self) -> Vec<Entity<InputState>> {
        vec![
            self.history_window_input.clone(),
            self.dedup_window_input.clone(),
            self.max_text_bytes_input.clone(),
            self.gc_batch_size_input.clone(),
        ]
    }

    pub(super) fn sync_from_workspace(
        &mut self,
        page: &SettingsStorageViewState,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        let next = StorageSyncedValues {
            history_window_days: page.history_window_days.to_string(),
            dedup_window_days: page.dedup_window_days.to_string(),
            max_text_bytes: page.max_text_bytes.to_string(),
            gc_batch_size: page.gc_batch_size.to_string(),
        };

        if let Some(previous) = self.synced.clone() {
            self.sync_input_if_clean(
                &self.history_window_input,
                &previous.history_window_days,
                &next.history_window_days,
                window,
                cx,
            );
            self.sync_input_if_clean(
                &self.dedup_window_input,
                &previous.dedup_window_days,
                &next.dedup_window_days,
                window,
                cx,
            );
            self.sync_input_if_clean(
                &self.max_text_bytes_input,
                &previous.max_text_bytes,
                &next.max_text_bytes,
                window,
                cx,
            );
            self.sync_input_if_clean(
                &self.gc_batch_size_input,
                &previous.gc_batch_size,
                &next.gc_batch_size,
                window,
                cx,
            );
        } else {
            self.force_sync_all(&next, window, cx);
        }

        self.synced = Some(next);
    }

    pub(super) fn history_window_input(&self) -> Entity<InputState> {
        self.history_window_input.clone()
    }

    pub(super) fn dedup_window_input(&self) -> Entity<InputState> {
        self.dedup_window_input.clone()
    }

    pub(super) fn max_text_bytes_input(&self) -> Entity<InputState> {
        self.max_text_bytes_input.clone()
    }

    pub(super) fn gc_batch_size_input(&self) -> Entity<InputState> {
        self.gc_batch_size_input.clone()
    }

    pub(super) fn history_window_value(&self, cx: &Context<WorkspaceView>) -> String {
        self.history_window_input.read(cx).value().to_string()
    }

    pub(super) fn dedup_window_value(&self, cx: &Context<WorkspaceView>) -> String {
        self.dedup_window_input.read(cx).value().to_string()
    }

    pub(super) fn max_text_bytes_value(&self, cx: &Context<WorkspaceView>) -> String {
        self.max_text_bytes_input.read(cx).value().to_string()
    }

    pub(super) fn gc_batch_size_value(&self, cx: &Context<WorkspaceView>) -> String {
        self.gc_batch_size_input.read(cx).value().to_string()
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
        next: &StorageSyncedValues,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        for (entity, value) in [
            (&self.history_window_input, next.history_window_days.clone()),
            (&self.dedup_window_input, next.dedup_window_days.clone()),
            (&self.max_text_bytes_input, next.max_text_bytes.clone()),
            (&self.gc_batch_size_input, next.gc_batch_size.clone()),
        ] {
            entity.update(cx, |input, cx| {
                input.set_value(value.clone(), window, cx);
            });
        }
    }
}
