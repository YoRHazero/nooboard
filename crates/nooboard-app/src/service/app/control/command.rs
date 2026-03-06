use tokio::sync::oneshot;

use crate::AppResult;
use crate::clipboard_runtime::{LocalClipboardObserved, LocalClipboardSubscription};
use crate::service::types::{
    AppPatch, AppServiceSnapshot, EventId, EventSubscription, FileDecisionRequest, HistoryPage,
    IngestTextRequest, ListHistoryRequest, RebroadcastEventRequest, SendFileRequest,
    SyncDesiredState,
};

pub(crate) enum ControlCommand {
    Shutdown {
        reply: oneshot::Sender<AppResult<()>>,
    },
    SetSyncDesiredState {
        desired_state: SyncDesiredState,
        reply: oneshot::Sender<AppResult<AppServiceSnapshot>>,
    },
    ApplyConfigPatch {
        patch: AppPatch,
        reply: oneshot::Sender<AppResult<AppServiceSnapshot>>,
    },
    Snapshot {
        reply: oneshot::Sender<AppResult<AppServiceSnapshot>>,
    },

    IngestTextEvent {
        request: IngestTextRequest,
        reply: oneshot::Sender<AppResult<()>>,
    },
    WriteEventToClipboard {
        event_id: EventId,
        reply: oneshot::Sender<AppResult<()>>,
    },
    ListHistory {
        request: ListHistoryRequest,
        reply: oneshot::Sender<AppResult<HistoryPage>>,
    },
    RebroadcastEvent {
        request: RebroadcastEventRequest,
        reply: oneshot::Sender<AppResult<()>>,
    },
    SetLocalWatchEnabled {
        enabled: bool,
        reply: oneshot::Sender<AppResult<()>>,
    },

    SendFile {
        request: SendFileRequest,
        reply: oneshot::Sender<AppResult<()>>,
    },
    RespondFileDecision {
        request: FileDecisionRequest,
        reply: oneshot::Sender<AppResult<()>>,
    },

    SubscribeEvents {
        reply: oneshot::Sender<AppResult<EventSubscription>>,
    },
    SubscribeLocalClipboard {
        reply: oneshot::Sender<AppResult<LocalClipboardSubscription>>,
    },
    InternalLocalClipboardObserved {
        observed: LocalClipboardObserved,
    },
    InternalSyncEvent {
        event: nooboard_sync::SyncEvent,
    },
}
