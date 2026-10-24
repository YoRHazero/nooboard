//! A WebView subscription never owns the core service lifetime.
use crate::{
    desktop,
    ipc::{dto, errors, snapshot::snapshot},
};
use tauri::ipc::Channel;
use tokio::sync::watch;

pub struct Subscription {
    pub id: String,
    task: tauri::async_runtime::JoinHandle<()>,
}
impl Drop for Subscription {
    fn drop(&mut self) {
        self.task.abort();
    }
}
pub fn start(
    id: String,
    mut backend: watch::Receiver<nooboard_core::AppSnapshot>,
    mut desktop: watch::Receiver<desktop::Snapshot>,
    diagnostic: bool,
    channel: Channel<dto::Frame>,
) -> (dto::Connection, Subscription) {
    let initial = snapshot(backend.borrow_and_update().clone());
    let host = desktop.borrow_and_update().clone();
    let mut visible = host.visible;
    let task = tauri::async_runtime::spawn(async move {
        loop {
            tokio::select! {
                changed = backend.changed() => {
                    if changed.is_err() { break; }
                    let current = backend.borrow_and_update().clone();
                    if visible && channel.send(dto::Frame::Snapshot(snapshot(current))).is_err() { return; }
                }
                changed = desktop.changed() => {
                    if changed.is_err() { return; }
                    let state = desktop.borrow_and_update().clone();
                    if state.visible && !visible
                        && channel.send(dto::Frame::Recovered(snapshot(backend.borrow_and_update().clone()))).is_err() { return; }
                    visible = state.visible;
                    if channel.send(dto::Frame::Host(state)).is_err() { return; }
                }
            }
        }
        let _ = channel.send(dto::Frame::Stopped(errors::ui("stopped")));
    });
    (
        dto::Connection {
            snapshot: initial,
            diagnostic,
            host,
        },
        Subscription { id, task },
    )
}
