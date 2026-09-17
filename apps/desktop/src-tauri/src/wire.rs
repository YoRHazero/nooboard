//! IPC serialization. Wide identifiers cross JavaScript as decimal strings.
use nooboard_core::{AppSnapshot, HistoryEntry};
use serde::Serialize;
use serde_json::Value;

pub fn snapshot(snapshot: AppSnapshot) -> Value {
    let pairing_error = snapshot
        .onboarding
        .session
        .as_ref()
        .and_then(|session| session.error.as_ref())
        .map(crate::errors::pairing);
    let mut value = serde_json::to_value(snapshot).expect("snapshot serialization is infallible");
    if let Some(session) = value["onboarding"]["session"].as_object_mut() {
        session.insert("error".into(), serde_json::to_value(pairing_error).unwrap());
    }
    for (pointer, code) in [
        ("/onboarding/discovery_error", "discovery"),
        ("/local_network/error", "localAddress"),
        ("/fault/message", "serviceFault"),
    ] {
        if let Some(error) = value.pointer_mut(pointer).filter(|v| !v.is_null()) {
            *error = serde_json::to_value(crate::errors::ui(code)).unwrap();
        }
    }
    value["platform"] = match std::env::consts::OS {
        "macos" => Value::String("macOS".into()),
        "windows" => Value::String("Windows".into()),
        _ => Value::Null,
    };
    for key in ["revision", "history_revision"] {
        decimal(&mut value[key]);
    }
    decimal(&mut value["current"]["revision"]);
    if let Some(fault) = value.get_mut("fault").filter(|v| !v.is_null()) {
        decimal(&mut fault["sequence"]);
    }
    for activity in value["activities"].as_array_mut().expect("activity array") {
        decimal(&mut activity["sequence"]);
        if !activity["message_id"].is_null() {
            decimal(&mut activity["message_id"]["sequence"]);
        }
    }
    for transfer in value["status"]["transfers"]
        .as_array_mut()
        .expect("transfer array")
    {
        decimal(&mut transfer["id"]["sequence"]);
    }
    for transfer in value["content_transfers"]
        .as_array_mut()
        .expect("content transfer array")
    {
        decimal(&mut transfer["id"]["sequence"]);
    }
    value
}
fn decimal(value: &mut Value) {
    *value = Value::String(value.as_u64().expect("unsigned identifier").to_string());
}
#[derive(Serialize)]
pub struct HistoryItem {
    id: String,
    text: String,
    source: String,
    copied_at_ms: i64,
}
impl From<HistoryEntry> for HistoryItem {
    fn from(row: HistoryEntry) -> Self {
        Self {
            id: row.id.to_string(),
            text: row.text,
            source: row.source,
            copied_at_ms: row.copied_at_ms,
        }
    }
}
#[derive(Serialize)]
pub struct HistoryPage {
    pub items: Vec<HistoryItem>,
    pub has_more: bool,
}
#[derive(Clone, Serialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum Frame {
    Snapshot(Value),
    Stopped(crate::errors::UiError),
}
#[derive(Serialize)]
pub struct Connection {
    pub snapshot: Value,
    pub diagnostic: bool,
}
#[cfg(test)]
mod tests {
    #[test]
    fn wide_identifiers_do_not_round_through_javascript_numbers() {
        let mut value = serde_json::json!(u64::MAX);
        super::decimal(&mut value);
        assert_eq!(value, "18446744073709551615");
    }
}
