use crate::ipc::contracts::*;
macro_rules! map_core_enum {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        impl From<nooboard_core::$name> for $name {
            fn from(value:nooboard_core::$name)->Self {match value {$(nooboard_core::$name::$variant=>Self::$variant),+}}
        }
    };
}
map_core_enum!(AppState {
    Running,
    Stopping,
    Stopped,
    Failed
});
map_core_enum!(ClipboardKind {
    Text,
    Image,
    Files,
    Empty,
    Unsupported,
    Sensitive,
    TooLarge
});
map_core_enum!(ContentKind { Image, Files });
map_core_enum!(ContentStage {
    Preparing,
    Queued,
    Waiting,
    Sending,
    Receiving,
    Verifying,
    Saving,
    Applying,
    Cancelling,
    Completed,
    Saved,
    Failed,
    Cancelled,
    Unconfirmed
});
map_core_enum!(TransferFailure {
    Denied,
    Directory,
    Unsupported,
    TooLarge,
    SourceChanged,
    Integrity,
    Io,
    Clipboard,
    Offline,
    Timeout,
    Busy,
    Cancelled,
    Protocol
});
map_core_enum!(DeliveryState {
    Queued,
    Sending,
    AwaitingReceipt,
    Applied,
    Rejected,
    Unconfirmed,
    Offline,
    Cancelled,
    Superseded,
    QueueFull
});
map_core_enum!(PairingStage {
    Requesting,
    AwaitingApproval,
    ShowingCode,
    EnteringCode,
    Verifying,
    Saving,
    Completed,
    Failed
});
map_core_enum!(ActivityKind {
    Copied,
    Sent,
    Received
});

impl From<nooboard_core::MessageId> for MessageId {
    fn from(id: nooboard_core::MessageId) -> Self {
        Self {
            session: id.session().into(),
            sequence: id.sequence().to_string(),
        }
    }
}
impl From<nooboard_core::PeerConnectionState> for PeerConnectionState {
    fn from(value: nooboard_core::PeerConnectionState) -> Self {
        match value {
            nooboard_core::PeerConnectionState::Connected => Self::Connected,
            nooboard_core::PeerConnectionState::Paused => Self::Paused,
            nooboard_core::PeerConnectionState::Offline => Self::Offline,
            nooboard_core::PeerConnectionState::Unconnected => Self::Unconnected,
            nooboard_core::PeerConnectionState::Unpaired => Self::Unpaired,
        }
    }
}
