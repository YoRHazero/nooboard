use crate::{
    error::{Error, Result},
    ipc::{contracts as dto, errors, mapping::snapshot::snapshot},
};
use std::sync::Mutex;
use tauri::ipc::Channel;
use tokio::sync::watch;
pub struct Sources {
    pub backend: watch::Receiver<nooboard_core::AppSnapshot>,
    pub desktop: watch::Receiver<crate::app::snapshot::Snapshot>,
    pub terminal: nooboard_core::terminal::TerminalWorkspace,
    pub diagnostic: bool,
}
#[derive(Default)]
struct Registration {
    generation: u64,
    desired: Option<String>,
    active: Option<Subscription>,
}
#[derive(Default)]
pub struct Subscriptions(Mutex<Registration>);
impl Subscriptions {
    pub fn begin(&self, id: String) -> Result<u64> {
        let mut state = self.0.lock().map_err(|_| Error::Subscription)?;
        state.generation = state.generation.wrapping_add(1);
        state.desired = Some(id);
        Ok(state.generation)
    }
    pub fn attach(
        &self,
        ticket: u64,
        id: String,
        sources: Sources,
        channel: Channel<dto::Frame>,
        io: &super::terminal_io::TerminalIo,
    ) -> Result<dto::Connection> {
        self.install(ticket, id, |id| {
            io.activate(id.clone());
            start(id, sources, channel)
        })
    }
    pub fn owns(&self, id: &str) -> bool {
        let state = self.0.lock().unwrap();
        state.desired.as_deref() == Some(id)
            && state.active.as_ref().is_some_and(|active| active.id == id)
    }
    fn install<T>(
        &self,
        ticket: u64,
        id: String,
        create: impl FnOnce(String) -> (T, Subscription),
    ) -> Result<T> {
        let mut state = self.0.lock().map_err(|_| Error::Subscription)?;
        if state.generation != ticket || state.desired.as_deref() != Some(&id) {
            return Err(Error::Subscription);
        }
        let (connection, subscription) = create(id);
        state.active = Some(subscription);
        Ok(connection)
    }
    pub fn disconnect(&self, id: &str) {
        if let Ok(mut state) = self.0.lock() {
            if state.desired.as_deref() == Some(id) {
                state.generation = state.generation.wrapping_add(1);
                state.desired = None;
            }
            if state.active.as_ref().is_some_and(|s| s.id == id) {
                state.active = None;
            }
        }
    }
}
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
    sources: Sources,
    channel: Channel<dto::Frame>,
) -> (dto::Connection, Subscription) {
    let Sources {
        mut backend,
        mut desktop,
        terminal,
        diagnostic,
    } = sources;
    let mut terminal_events = terminal.subscribe_runs().events;
    let mut terminal_watching = true;
    let initial = snapshot(backend.borrow_and_update().clone());
    let host = crate::ipc::mapping::desktop::snapshot(desktop.borrow_and_update().clone());
    let mut visible = host.visible;
    let task = tauri::async_runtime::spawn(async move {
        loop {
            tokio::select! {
                event=terminal_events.next(),if terminal_watching=>match event {
                    Ok(Some(event))=>if channel.send(dto::Frame::TerminalEnded(super::mapping::terminal::ended(event))).is_err(){return;},
                    Err(_)=>{let _=channel.send(dto::Frame::TerminalLagged);terminal_events=terminal.subscribe_runs().events;},
                    Ok(None)=>terminal_watching=false,
                },
                changed = backend.changed() => {
                    if changed.is_err() { break; }
                    let current = backend.borrow_and_update().clone();
                    if visible && channel.send(dto::Frame::Snapshot(snapshot(current))).is_err() { return; }
                }
                changed = desktop.changed() => {
                    if changed.is_err() { return; }
                    let state = crate::ipc::mapping::desktop::snapshot(desktop.borrow_and_update().clone());
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

#[cfg(test)]
mod tests {
    use super::*;
    fn idle(id: String) -> ((), Subscription) {
        (
            (),
            Subscription {
                id,
                task: tauri::async_runtime::spawn(std::future::pending()),
            },
        )
    }
    #[tokio::test]
    async fn pending_cancellation_and_late_connections_cannot_replace_the_new_subscription() {
        let subscriptions = Subscriptions::default();
        let old = subscriptions.begin("old".into()).unwrap();
        subscriptions.disconnect("old");
        assert!(
            subscriptions
                .install::<()>(old, "old".into(), |_| panic!(
                    "cancelled connection spawned a pump"
                ))
                .is_err()
        );
        let old = subscriptions.begin("old".into()).unwrap();
        let current = subscriptions.begin("current".into()).unwrap();
        subscriptions
            .install(current, "current".into(), idle)
            .unwrap();
        assert!(
            subscriptions
                .install::<()>(old, "old".into(), |_| panic!(
                    "late connection spawned a pump"
                ))
                .is_err()
        );
        subscriptions.disconnect("old");
        assert_eq!(
            subscriptions.0.lock().unwrap().active.as_ref().unwrap().id,
            "current"
        );
        subscriptions.disconnect("current");
        assert!(subscriptions.0.lock().unwrap().active.is_none());
    }
}
