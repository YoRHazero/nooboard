mod commands;
pub mod dto;
pub mod errors;
pub mod snapshot;
use crate::host::Host;
use tauri::{AppHandle, State, ipc::Channel};

#[tauri::command]
pub async fn connect(
    handle: AppHandle,
    host: State<'_, Host>,
    subscription: String,
    on_frame: Channel<dto::Frame>,
) -> Result<dto::Connection, errors::UiError> {
    if subscription.is_empty() || subscription.len() > 128 {
        return Err(errors::ui("subscription"));
    }
    host.connect(&handle, subscription, on_frame).await
}
#[tauri::command]
pub fn disconnect(host: State<'_, Host>, subscription: String) {
    host.disconnect(&subscription);
}
#[tauri::command]
pub async fn request(
    handle: AppHandle,
    host: State<'_, Host>,
    request: dto::Request,
) -> Result<dto::Reply, errors::UiError> {
    commands::run(&handle, &host, request).await
}

#[cfg(test)]
mod tests {
    #[test]
    fn ipc_schema_matches_rust_contract() {
        let schema = schemars::schema_for!(super::dto::Contract);
        let generated = serde_json::to_string_pretty(&schema).unwrap() + "\n";
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../ipc/schema.json");
        if std::env::var("NOOBOARD_UPDATE_IPC").as_deref() == Ok("1") {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, generated).unwrap();
        } else {
            assert_eq!(
                std::fs::read_to_string(path).unwrap(),
                generated,
                "Regenerate the IPC contract with npm run ipc:generate"
            );
        }
    }
    #[test]
    fn write_requests_reject_read_only_fields_and_keep_wide_ids_as_strings() {
        use super::dto::Request;
        assert!(
            serde_json::from_str::<Request>(
                r#"{"type":"updateSyncSettings","data":{"patch":{"restartRequired":true}}}"#
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<Request>(
                r#"{"type":"updateSyncSettings","data":{"patch":{"theme":"dark"}}}"#
            )
            .is_err()
        );
        let request: Request =
            serde_json::from_str(r#"{"type":"copyHistory","data":{"id":"9223372036854775807"}}"#)
                .unwrap();
        assert!(matches!(request, Request::CopyHistory { id } if id == i64::MAX.to_string()));
        let id = super::dto::MessageId {
            session: "session".into(),
            sequence: u64::MAX.to_string(),
        };
        assert_eq!(
            serde_json::to_value(id).unwrap()["sequence"],
            "18446744073709551615"
        );
    }
}
