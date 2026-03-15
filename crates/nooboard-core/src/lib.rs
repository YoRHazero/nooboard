mod bootstrap;
mod clipboard;
mod error;
mod runtime;
mod storage;
mod types;
mod workspace;

pub use bootstrap::{
    BootstrapChooserContext, BootstrapChooserReason, BootstrapDecision, BootstrapLaunch,
    BootstrapMode, BootstrapRequest, CustomLocationProbe, ExistingConfigProbe,
    RepoDevelopmentProbe, inspect_custom_location, inspect_existing_config,
    inspect_repo_development, prepare_custom_location_launch, prepare_default_config_from_chooser,
    prepare_existing_config_launch, prepare_repo_development_launch, resolve_bootstrap,
    rewrite_existing_config,
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
