use super::*;
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub id: String,
    pub text: String,
    pub source: String,
    pub copied_at: i64,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPage {
    pub items: Vec<HistoryItem>,
    pub has_more: bool,
}
#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum HistorySource {
    All,
    Local,
    Remote,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HistoryQuery {
    pub contains: String,
    pub source: HistorySource,
    pub offset: u32,
}
