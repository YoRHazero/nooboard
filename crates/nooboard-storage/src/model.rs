#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardRecord {
    pub id: i64,
    pub content: String,
    pub captured_at: i64,
}
