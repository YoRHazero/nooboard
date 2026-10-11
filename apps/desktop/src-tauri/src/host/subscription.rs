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
    remote: nooboard_core::Remote,
    mut desktop: watch::Receiver<desktop::Snapshot>,
    diagnostic: bool,
    channel: Channel<dto::Frame>,
) -> (dto::Connection, Subscription) {
    let mut terminal = remote.subscribe_terminal();
    let mut remote_state = remote.subscribe();
    let mut previous_remote = remote_state.borrow_and_update().clone();
    let remote_initial = previous_remote.clone().into();
    let initial = snapshot(backend.borrow_and_update().clone());
    let host = desktop.borrow_and_update().clone();
    let mut visible = host.visible;
    let task = tauri::async_runtime::spawn(async move {
        let mut remote_dirty = false;
        let mut remote_tick = tokio::time::interval(std::time::Duration::from_millis(50));
        remote_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                changed = remote_state.changed() => {
                    if changed.is_err() { break; }
                    remote_dirty = true;
                }
                _ = remote_tick.tick(), if remote_dirty => {
                    remote_dirty = false;
                    let latest = remote_state.borrow_and_update().clone();
                    let frame = crate::ipc::remote::RemoteSnapshot::project(latest.clone(), Some(&previous_remote));
                    previous_remote = latest;
                    if channel.send(dto::Frame::Remote(frame)).is_err() { return; }
                }
                output = terminal.recv() => {
                    match output {
                        Ok(chunk) if visible => { if channel.send(dto::Frame::Terminal(crate::ipc::remote::TerminalFrame { session_id:chunk.session_id,offset:chunk.offset.to_string(),bytes:chunk.bytes })).is_err() { return; } }
                        Ok(_) => {},
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => { if channel.send(dto::Frame::TerminalReset).is_err() { return; } }
                        Err(_) => break,
                    }
                }
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
                    if state.visible && !visible && channel.send(dto::Frame::TerminalReset).is_err() { return; }
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
            remote: remote_initial,
            diagnostic,
            host,
        },
        Subscription { id, task },
    )
}
