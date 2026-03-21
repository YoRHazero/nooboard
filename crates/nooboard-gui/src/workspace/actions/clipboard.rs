use gpui::{Context, Entity, Task};
use nooboard_core::{
    ClipboardHistoryPage, ClipboardRecord, EventId, ListClipboardHistoryRequest, SessionTarget,
};

use crate::workspace::controller::WorkspaceController;

use super::spawn::spawn_core_call;

pub fn submit_text_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    content: String,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<EventId>>> {
    spawn_core_call(
        controller,
        cx,
        "couldn't save a clipboard item",
        move |core| async move { core.submit_text(content).await },
    )
}

pub fn get_clipboard_record_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    event_id: EventId,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<ClipboardRecord>>> {
    spawn_core_call(
        controller,
        cx,
        "couldn't load a clipboard item",
        move |core| async move { core.get_clipboard_record(event_id).await },
    )
}

pub fn list_clipboard_history_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    request: ListClipboardHistoryRequest,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<ClipboardHistoryPage>>> {
    spawn_core_call(
        controller,
        cx,
        "couldn't load clipboard history",
        move |core| async move { core.list_clipboard_history(request).await },
    )
}

pub fn adopt_clipboard_record_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    event_id: EventId,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<()>>> {
    spawn_core_call(
        controller,
        cx,
        "couldn't copy a clipboard item",
        move |core| async move { core.adopt_clipboard_record(event_id).await },
    )
}

pub fn rebroadcast_clipboard_record_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    event_id: EventId,
    target: SessionTarget,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<()>>> {
    spawn_core_call(
        controller,
        cx,
        "couldn't send a clipboard item",
        move |core| async move { core.rebroadcast_clipboard_record(event_id, target).await },
    )
}
