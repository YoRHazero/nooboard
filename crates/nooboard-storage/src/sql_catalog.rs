#[derive(Debug, Clone)]
pub struct SqlCatalog {
    pub schema: String,
    pub insert_event: String,
    pub select_latest_active_content: String,
    pub select_event_by_id: String,
    pub list_history: String,
    pub list_history_with_cursor: String,
    pub search_history: String,
    pub gc_mark_tombstone: String,
    pub gc_delete_expired_tombstone: String,
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
            select_event_by_id: include_str!("../sql/queries/select_event_by_id.sql").to_string(),
            list_history: include_str!("../sql/queries/list_history.sql").to_string(),
            list_history_with_cursor: include_str!("../sql/queries/list_history_with_cursor.sql")
                .to_string(),
            search_history: include_str!("../sql/queries/search_history.sql").to_string(),
            gc_mark_tombstone: include_str!("../sql/queries/gc_mark_tombstone.sql").to_string(),
            gc_delete_expired_tombstone: include_str!(
                "../sql/queries/gc_delete_expired_tombstone.sql"
            )
            .to_string(),
        }
    }
}
