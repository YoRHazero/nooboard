use tokio::sync::oneshot;

use crate::AppResult;
use crate::clipboard_runtime::LocalClipboardObserved;
use crate::service::types::{
    AppState, ClipboardHistoryPage, ClipboardRecord, EventId, EventSubscription,
    IncomingTransferDecision, ListClipboardHistoryRequest, RebroadcastClipboardRequest,
    SendFilesRequest, SettingsPatch, StateSubscription, SubmitTextRequest, SyncDesiredState,
    TransferId,
};

pub(crate) enum ControlCommand {
    Shutdown {
        reply: oneshot::Sender<AppResult<()>>,
    },
    GetState {
        reply: oneshot::Sender<AppResult<AppState>>,
    },
    SubscribeState {
        reply: oneshot::Sender<AppResult<StateSubscription>>,
    },
    SubscribeEvents {
        reply: oneshot::Sender<AppResult<EventSubscription>>,
    },
    SetSyncDesiredState {
        desired_state: SyncDesiredState,
        reply: oneshot::Sender<AppResult<()>>,
    },
    PatchSettings {
        patch: SettingsPatch,
        reply: oneshot::Sender<AppResult<()>>,
    },
    SubmitText {
        request: SubmitTextRequest,
        reply: oneshot::Sender<AppResult<EventId>>,
    },
    GetClipboardRecord {
        event_id: EventId,
        reply: oneshot::Sender<AppResult<ClipboardRecord>>,
    },
    ListClipboardHistory {
        request: ListClipboardHistoryRequest,
        reply: oneshot::Sender<AppResult<ClipboardHistoryPage>>,
    },
    AdoptClipboardRecord {
        event_id: EventId,
        reply: oneshot::Sender<AppResult<()>>,
    },
    RebroadcastClipboardRecord {
        request: RebroadcastClipboardRequest,
        reply: oneshot::Sender<AppResult<()>>,
    },
    SendFiles {
        request: SendFilesRequest,
        reply: oneshot::Sender<AppResult<Vec<TransferId>>>,
    },
    DecideIncomingTransfer {
        request: IncomingTransferDecision,
        reply: oneshot::Sender<AppResult<()>>,
    },
    CancelTransfer {
        transfer_id: TransferId,
        reply: oneshot::Sender<AppResult<()>>,
    },
    InternalLocalClipboardObserved {
        observed: LocalClipboardObserved,
    },
    InternalSyncEvent {
        event: nooboard_sync::SyncEvent,
    },
    InternalTransferUpdate {
        update: nooboard_sync::TransferUpdate,
    },
    InternalSyncStatusChanged {
        status: nooboard_sync::SyncStatus,
    },
    InternalConnectedPeersChanged {
        peers: Vec<nooboard_sync::ConnectedPeerInfo>,
    },
}
