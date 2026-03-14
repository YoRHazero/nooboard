mod bootstrap;
mod clipboard;
mod error;
mod runtime;
mod storage;
mod types;
mod workspace;

pub use bootstrap::{
    BootstrapChooserContext, BootstrapChooserReason, BootstrapDecision, BootstrapLaunch,
    BootstrapMode, BootstrapRequest, prepare_default_config_from_chooser, resolve_bootstrap,
};
pub use clipboard::port::ClipboardPort;
pub use error::{CoreError, CoreResult};
pub use nooboard_network::{
    ConnectDirectOutcome, ConnectionFailure, ConnectionFailureKind, DirectRequestId, DirectSeedId,
    DirectSeedInfo, IncomingTransferDecision, IncomingTransferDisposition, IncomingTransferOffer,
    LanPeerInfo, NetworkSnapshot, NetworkStatus, PendingDirectRequest, SendFilesRequest, SessionId,
    SessionInfo, SessionTarget, TransferOutcome, TransferTicket, TransfersSnapshot,
    UpsertDirectSeedInput,
};
pub use runtime::NooboardCore;
pub use types::{
    ClipboardHistoryCursor, ClipboardHistoryPage, ClipboardRecord, ClipboardRecordSource,
    ClipboardSettings, ClipboardState, ConnectionSettings, EventId, EventRecvError,
    EventSubscription, ListClipboardHistoryRequest, LocalConnectionInfo, NetworkSettings, NoobId,
    StateRecvError, StateSubscription, StorageSettings, StorageSettingsInput, TransferSettings,
    WorkspaceEvent, WorkspaceIdentity, WorkspaceSettings, WorkspaceSnapshot,
};
