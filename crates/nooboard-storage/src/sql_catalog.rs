#[derive(Debug, Clone)]
pub struct SqlCatalog {
    pub schema: String,
    pub insert_event: String,
    pub select_latest_active_content: String,
    pub list_history: String,
    pub list_history_with_cursor: String,
    pub search_history: String,
    pub gc_mark_tombstone: String,
    pub gc_delete_expired_tombstone: String,
    pub outbox_enqueue: String,
    pub outbox_list_due: String,
    pub outbox_try_lease: String,
    pub outbox_mark_sent: String,
    pub outbox_mark_retry: String,
}

impl SqlCatalog {
    pub fn load() -> Self {
        Self {
            schema: include_str!("../sql/bootstrap/schema.sql").to_string(),
            insert_event: include_str!("../sql/queries/insert_event.sql").to_string(),
            select_latest_active_content: include_str!(
                "../sql/queries/select_latest_active_content.sql"
            )
            .to_string(),
            list_history: include_str!("../sql/queries/list_history.sql").to_string(),
            list_history_with_cursor: include_str!("../sql/queries/list_history_with_cursor.sql")
                .to_string(),
            search_history: include_str!("../sql/queries/search_history.sql").to_string(),
            gc_mark_tombstone: include_str!("../sql/queries/gc_mark_tombstone.sql").to_string(),
            gc_delete_expired_tombstone: include_str!(
                "../sql/queries/gc_delete_expired_tombstone.sql"
            )
            .to_string(),
            outbox_enqueue: include_str!("../sql/queries/outbox_enqueue.sql").to_string(),
            outbox_list_due: include_str!("../sql/queries/outbox_list_due.sql").to_string(),
            outbox_try_lease: include_str!("../sql/queries/outbox_try_lease.sql").to_string(),
            outbox_mark_sent: include_str!("../sql/queries/outbox_mark_sent.sql").to_string(),
            outbox_mark_retry: include_str!("../sql/queries/outbox_mark_retry.sql").to_string(),
        }
    }
}
