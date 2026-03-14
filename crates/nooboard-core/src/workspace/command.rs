use tokio::sync::oneshot;

use crate::clipboard::LocalClipboardObserved;
use crate::error::CoreResult;
use crate::types::{
    ClipboardHistoryPage, ClipboardRecord, EventId, EventSubscription, ListClipboardHistoryRequest,
    StateSubscription, WorkspaceSnapshot,
};
use crate::{
    ConnectDirectOutcome, DirectRequestId, DirectSeedId, DirectSeedInfo, IncomingTransferDecision,
    PendingDirectRequest, SendFilesRequest, SessionId, SessionInfo, SessionTarget, TransferTicket,
    UpsertDirectSeedInput,
};

pub(crate) enum WorkspaceCommand {
    Shutdown {
        reply: oneshot::Sender<CoreResult<()>>,
    },
    Snapshot {
        reply: oneshot::Sender<CoreResult<WorkspaceSnapshot>>,
    },
    SubscribeState {
        reply: oneshot::Sender<CoreResult<StateSubscription>>,
    },
    SubscribeEvents {
        reply: oneshot::Sender<CoreResult<EventSubscription>>,
    },
    StartNetwork {
        reply: oneshot::Sender<CoreResult<()>>,
    },
    StopNetwork {
        reply: oneshot::Sender<CoreResult<()>>,
    },
    SetDeviceId {
        value: String,
        reply: oneshot::Sender<CoreResult<()>>,
    },
    SetNetworkToken {
        value: String,
        reply: oneshot::Sender<CoreResult<()>>,
    },
    SetNetworkListenPort {
        value: u16,
        reply: oneshot::Sender<CoreResult<()>>,
    },
    SetLanEnabled {
        value: bool,
        reply: oneshot::Sender<CoreResult<()>>,
    },
    SetLocalCaptureEnabled {
        value: bool,
        reply: oneshot::Sender<CoreResult<()>>,
    },
    SetDownloadDir {
        value: std::path::PathBuf,
        reply: oneshot::Sender<CoreResult<()>>,
    },
    SetStorageSettings {
        input: crate::StorageSettingsInput,
        reply: oneshot::Sender<CoreResult<()>>,
    },
    ListDirectSeeds {
        reply: oneshot::Sender<CoreResult<Vec<DirectSeedInfo>>>,
    },
    UpsertDirectSeed {
        input: UpsertDirectSeedInput,
        reply: oneshot::Sender<CoreResult<DirectSeedId>>,
    },
    RemoveDirectSeed {
        id: DirectSeedId,
        reply: oneshot::Sender<CoreResult<()>>,
    },
    SearchDirectSeeds {
        query: String,
        reply: oneshot::Sender<CoreResult<Vec<DirectSeedInfo>>>,
    },
    ConnectDirectSeed {
        id: DirectSeedId,
        reply: oneshot::Sender<CoreResult<ConnectDirectOutcome>>,
    },
    ListPendingDirectRequests {
        reply: oneshot::Sender<CoreResult<Vec<PendingDirectRequest>>>,
    },
    ApproveDirectRequest {
        id: DirectRequestId,
        reply: oneshot::Sender<CoreResult<()>>,
    },
    RejectDirectRequest {
        id: DirectRequestId,
        reply: oneshot::Sender<CoreResult<()>>,
    },
    ListSessions {
        reply: oneshot::Sender<CoreResult<Vec<SessionInfo>>>,
    },
    DisconnectSession {
        id: SessionId,
        reply: oneshot::Sender<CoreResult<()>>,
    },
    SubmitText {
        content: String,
        reply: oneshot::Sender<CoreResult<EventId>>,
    },
    GetClipboardRecord {
        event_id: EventId,
        reply: oneshot::Sender<CoreResult<ClipboardRecord>>,
    },
    ListClipboardHistory {
        request: ListClipboardHistoryRequest,
        reply: oneshot::Sender<CoreResult<ClipboardHistoryPage>>,
    },
    AdoptClipboardRecord {
        event_id: EventId,
        reply: oneshot::Sender<CoreResult<()>>,
    },
    RebroadcastClipboardRecord {
        event_id: EventId,
        target: SessionTarget,
        reply: oneshot::Sender<CoreResult<()>>,
    },
    SendFiles {
        request: SendFilesRequest,
        reply: oneshot::Sender<CoreResult<Vec<TransferTicket>>>,
    },
    DecideIncomingTransfer {
        decision: IncomingTransferDecision,
        reply: oneshot::Sender<CoreResult<()>>,
    },
    CancelTransfer {
        ticket: TransferTicket,
        reply: oneshot::Sender<CoreResult<()>>,
    },
    Bridge(WorkspaceBridgeMessage),
}

pub(crate) enum WorkspaceBridgeMessage {
    LocalClipboardObserved(LocalClipboardObserved),
    NetworkEvent(nooboard_network::NetworkEvent),
}
