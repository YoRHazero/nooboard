use super::{
    contracts,
    errors::{self, UiError},
};
use crate::{app::runtime::Runtime, error::Error};
use std::sync::Arc;
use tauri::{State, ipc::Channel};
#[tauri::command]
pub async fn connect(
    runtime: State<'_, Arc<Runtime>>,
    subscription: String,
    on_frame: Channel<contracts::Frame>,
) -> Result<contracts::Connection, UiError> {
    if subscription.is_empty() || subscription.len() > 128 {
        return Err(errors::ui("subscription"));
    }
    if runtime.lifecycle.control.exiting() {
        return Err(errors::desktop(Error::Closing));
    }
    let ticket = runtime
        .subscriptions
        .begin(subscription.clone())
        .map_err(errors::desktop)?;
    runtime
        .backend
        .ensure_started()
        .await
        .map_err(errors::desktop)?;
    let backend = runtime
        .backend
        .connection()
        .await
        .map_err(errors::desktop)?;
    if runtime.lifecycle.control.exiting() {
        return Err(errors::desktop(Error::Closing));
    }
    runtime.publish();
    runtime
        .subscriptions
        .attach(
            ticket,
            subscription,
            backend.app.subscribe(),
            runtime.subscribe(),
            backend.diagnostic,
            on_frame,
        )
        .map_err(errors::desktop)
}
#[tauri::command]
pub fn disconnect(runtime: State<'_, Arc<Runtime>>, subscription: String) {
    runtime.subscriptions.disconnect(&subscription);
}
#[tauri::command]
pub async fn request(
    runtime: State<'_, Arc<Runtime>>,
    request: contracts::Request,
) -> Result<contracts::Reply, UiError> {
    super::router::run(runtime.inner(), request).await
}
