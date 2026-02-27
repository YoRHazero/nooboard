use tokio::sync::oneshot;

use crate::AppResult;
use crate::service::types::{
    AppPatch, AppServiceSnapshot, EventId, EventSubscription, FileDecisionRequest, HistoryPage,
    ListHistoryRequest, LocalClipboardChangeRequest, LocalClipboardChangeResult,
    RebroadcastHistoryRequest, RemoteTextRequest, SendFileRequest, SyncDesiredState,
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

    ApplyLocalClipboardChange {
        request: LocalClipboardChangeRequest,
        reply: oneshot::Sender<AppResult<LocalClipboardChangeResult>>,
    },
    ApplyHistoryEntryToClipboard {
        event_id: EventId,
        reply: oneshot::Sender<AppResult<()>>,
    },
    ListHistory {
        request: ListHistoryRequest,
        reply: oneshot::Sender<AppResult<HistoryPage>>,
    },
    RebroadcastHistoryEntry {
        request: RebroadcastHistoryRequest,
        reply: oneshot::Sender<AppResult<()>>,
    },
    StoreRemoteText {
        request: RemoteTextRequest,
        reply: oneshot::Sender<AppResult<()>>,
    },
    WriteRemoteTextToClipboard {
        request: RemoteTextRequest,
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

    TickOutbox,
}
