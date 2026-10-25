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
            super::subscription::Sources {
                backend: backend.app.subscribe(),
                desktop: runtime.subscribe(),
                terminal: backend.app.terminal(),
                diagnostic: backend.diagnostic,
            },
            on_frame,
            &runtime.terminal_io,
        )
        .map_err(errors::desktop)
}
#[tauri::command]
pub fn disconnect(runtime: State<'_, Arc<Runtime>>, subscription: String) {
    runtime.subscriptions.disconnect(&subscription);
    runtime.terminal_io.deactivate(&subscription);
}
#[tauri::command]
pub async fn request(
    runtime: State<'_, Arc<Runtime>>,
    request: contracts::Request,
    webview: tauri::Webview,
    on_output: Option<tauri::ipc::JavaScriptChannelId>,
) -> Result<contracts::Reply, UiError> {
    if let contracts::Request::Terminal { owner, action } = request {
        if !runtime.subscriptions.owns(&owner) {
            return Err(errors::ui("stopped"));
        }
        if runtime.lifecycle.control.exiting() {
            return Err(errors::ui("closing"));
        }
        return super::terminal_requests::run(
            runtime.inner(),
            owner,
            action,
            on_output.map(|channel| channel.channel_on(webview)),
        )
        .await;
    }
    super::router::run(runtime.inner(), request).await
}
