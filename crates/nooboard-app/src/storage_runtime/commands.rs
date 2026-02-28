use nooboard_storage::{HistoryCursor, HistoryRecord};
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
    ListHistory {
        limit: usize,
        cursor: Option<HistoryCursor>,
        reply: oneshot::Sender<AppResult<Vec<HistoryRecord>>>,
    },
    Shutdown,
}
