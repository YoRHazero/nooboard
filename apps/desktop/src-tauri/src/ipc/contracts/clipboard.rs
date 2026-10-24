use super::*;
core_enum!(ClipboardKind {
    Text,
    Image,
    Files,
    Empty,
    Unsupported,
    Sensitive,
    TooLarge
});
core_enum!(ActivityKind {
    Copied,
    Sent,
    Received
});

#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Clipboard {
    pub revision: String,
    pub kind: ClipboardKind,
    pub text: Option<String>,
    pub source: Option<String>,
    pub copied_at: i64,
    pub files: Vec<String>,
    pub preview: Option<String>,
    pub image_width: Option<u32>,
    pub image_height: Option<u32>,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivityRecord {
    pub sequence: String,
    pub kind: ActivityKind,
    pub summary: String,
    pub at: i64,
    pub source: Option<String>,
    pub device_name: Option<String>,
    pub message_id: Option<MessageId>,
    pub content_task: Option<String>,
    pub content_node: Option<String>,
    pub content_stage: Option<ContentStage>,
}
