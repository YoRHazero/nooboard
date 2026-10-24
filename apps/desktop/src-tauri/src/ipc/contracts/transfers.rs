use super::*;
core_enum!(ContentKind { Image, Files });
core_enum!(ContentStage {
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
core_enum!(TransferFailure {
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
core_enum!(DeliveryState {
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
#[derive(Clone, Serialize, JsonSchema)]
pub struct MessageId {
    pub session: String,
    pub sequence: String,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Delivery {
    pub noob_id: String,
    pub device_name: String,
    pub state: DeliveryState,
}
#[derive(Clone, Serialize, JsonSchema)]
pub struct Transfer {
    pub id: MessageId,
    pub automatic: bool,
    pub bytes: usize,
    pub targets: Vec<Delivery>,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContentTransfer {
    pub key: String,
    pub id: MessageId,
    pub peer: String,
    pub device_name: String,
    pub incoming: bool,
    pub kind: ContentKind,
    pub names: Vec<String>,
    pub total_bytes: u64,
    pub completed_bytes: u64,
    pub stage: ContentStage,
    pub error: Option<TransferFailure>,
    pub saved_paths: Vec<String>,
    pub at: i64,
}
