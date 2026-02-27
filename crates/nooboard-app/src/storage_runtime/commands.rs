use nooboard_storage::{HistoryCursor, HistoryRecord, OutboxMessage};
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::AppResult;

pub(super) enum StorageCommand {
    Reconfigure {
        storage_config: nooboard_storage::AppConfig,
        reply: oneshot::Sender<AppResult<()>>,
    },
    AppendText {
        text: String,
        event_id: Option<Uuid>,
        origin_device_id: Option<String>,
        created_at_ms: i64,
        applied_at_ms: i64,
        reply: oneshot::Sender<AppResult<bool>>,
    },
    AppendTextWithOutbox {
        text: String,
        event_id: Uuid,
        origin_device_id: Option<String>,
        created_at_ms: i64,
        applied_at_ms: i64,
        targets: Option<Vec<String>>,
        enqueue_at_ms: i64,
        reply: oneshot::Sender<AppResult<bool>>,
    },
    ListHistory {
        limit: usize,
        cursor: Option<HistoryCursor>,
        reply: oneshot::Sender<AppResult<Vec<HistoryRecord>>>,
    },
    ListDueOutbox {
        now_ms: i64,
        limit: usize,
        reply: oneshot::Sender<AppResult<Vec<OutboxMessage>>>,
    },
    TryLeaseOutbox {
        id: i64,
        lease_until_ms: i64,
        now_ms: i64,
        reply: oneshot::Sender<AppResult<bool>>,
    },
    MarkOutboxSent {
        id: i64,
        sent_at_ms: i64,
        reply: oneshot::Sender<AppResult<bool>>,
    },
    MarkOutboxRetry {
        id: i64,
        next_attempt_at_ms: i64,
        error: String,
        now_ms: i64,
        reply: oneshot::Sender<AppResult<bool>>,
    },
    Shutdown,
}
