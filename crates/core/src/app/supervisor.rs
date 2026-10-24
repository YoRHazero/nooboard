//! Composition and supervision only; feature runtimes own their business state.
use super::{routing, shutdown};
use crate::{
    app::snapshot::assemble::{Inputs, assemble},
    configuration::application::runtime as configuration,
    features::clipboard::application::runtime as sync,
    features::devices::application::runtime as devices,
    features::history::application::runtime as history,
    *,
};
use nooboard_network::{Network, NetworkEvents};
use nooboard_storage::Storage;
use tokio::{
    sync::{broadcast, watch},
    task::JoinSet,
};
pub(crate) struct Running {
    pub(super) terminal: crate::terminal::TerminalWorkspace,
    pub(super) terminal_owner: std::sync::Arc<
        tokio::sync::Mutex<crate::features::terminal::application::service::TerminalFeatureService>,
    >,
    pub(super) services: shutdown::Services,
    pub(super) stop: watch::Sender<bool>,
    pub(super) stopped: watch::Receiver<bool>,
    pub(super) persist_stop: watch::Sender<bool>,
    pub(super) persist_stopped: watch::Receiver<bool>,
    pub(super) config: configuration::Handle,
    pub(super) config_runtime: configuration::Runtime,
    pub(super) history: history::Handle,
    pub(super) history_runtime: history::Runtime,
    pub(super) device: devices::Channels,
    pub(super) device_runtime: devices::Runtime,
    pub(super) sync: sync::Channels,
    pub(super) sync_runtime: sync::Runtime,
    pub(super) storage: Storage,
    pub(super) network: Network,
    pub(super) network_events: NetworkEvents,
    pub(super) snapshots: watch::Sender<AppSnapshot>,
    pub(super) events: broadcast::Sender<Event>,
    pub(super) session: String,
    pub(super) startup: Settings,
}
impl Running {
    pub async fn run(self) -> Result<()> {
        let Self {
            terminal,
            terminal_owner,
            mut services,
            stop,
            mut stopped,
            persist_stop,
            persist_stopped,
            config,
            config_runtime,
            history,
            history_runtime,
            device,
            device_runtime,
            sync,
            sync_runtime,
            storage,
            network,
            network_events,
            snapshots,
            events,
            session,
            startup,
        } = self;
        struct Stop(watch::Sender<bool>);
        impl Drop for Stop {
            fn drop(&mut self) {
                self.0.send_replace(true);
            }
        }
        let _guard = Stop(stop.clone());
        let _persistence_guard = Stop(persist_stop.clone());
        // Keep request channels alive independently of all public App clones.
        let _requests = (
            device.requests,
            sync.requests,
            config.requests.clone(),
            history.requests.clone(),
        );
        let mut work = JoinSet::new();
        let mut persistence = JoinSet::new();
        persistence.spawn(config_runtime.run(
            crate::configuration::adapters::storage::port(storage.clone()),
            persist_stopped.clone(),
        ));
        persistence.spawn(history_runtime.run(
            crate::features::history::adapters::storage::port(storage),
            std::sync::Arc::new(config.clone()),
            persist_stopped,
        ));
        work.spawn(device_runtime.run(stopped.clone()));
        work.spawn(sync_runtime.run(stopped.clone()));
        work.spawn(routing::run(
            network.clone(),
            network_events,
            device.events,
            sync.events,
            stopped.clone(),
        ));
        let mut t = terminal.subscribe();
        let mut terminal_watching = true;
        let mut c = config.state;
        let mut h = history.state;
        let mut d = device.state;
        let mut s = sync.state;
        let mut n = network.subscribe();
        let mut revision = 0;
        let mut failure = None;
        loop {
            if *stopped.borrow() {
                break;
            }
            tokio::select! {
                _ = stopped.changed() => break,
                result = work.join_next() => {
                    if !*stopped.borrow() { failure = Some(task_error(result)); }
                    break;
                }
                result = persistence.join_next() => {
                    failure = Some(task_error(result));
                    break;
                }
                _ = c.changed() => {}
                _ = h.changed() => {}
                _ = d.changed() => {}
                _ = s.changed() => {}
                _ = n.changed() => {}
                result = t.changed(), if terminal_watching => {
                    if result.is_err() {
                        terminal_watching = false;
                        if !t.borrow().closing { failure = Some(Error::Stopped); break; }
                    }
                }
            }
            revision += 1;
            let mut next = assemble(
                Inputs {
                    configuration: &c.borrow_and_update(),
                    devices: &d.borrow_and_update(),
                    sync: &s.borrow_and_update(),
                    history: &h.borrow_and_update(),
                    network: &n.borrow_and_update(),
                    startup: &startup,
                    terminal: &t.borrow_and_update(),
                },
                &session,
                revision,
                AppState::Running,
            );
            // These notifications are advisory; the snapshot is the recoverable source of truth.
            let old = snapshots.borrow().clone();
            if next.fault.as_ref().map(|f| &f.message) == old.fault.as_ref().map(|f| &f.message) {
                next.fault = old.fault.clone();
            }
            for transfer in &next.status.transfers {
                if !old.status.transfers.contains(transfer) {
                    let _ = events.send(Event::Transfer(transfer.clone()));
                }
            }
            if old.history_revision != next.history_revision {
                let _ = events.send(Event::HistoryChanged);
            }
            snapshots.send_replace(next);
            if !matches!(
                network.status().state,
                nooboard_network::ServiceState::Running
            ) {
                failure = Some(Error::Stopped);
                break;
            }
        }
        stop.send_replace(true);
        snapshots.send_modify(|s| s.status.state = AppState::Stopping);
        // First finish business tasks while the underlying services can still answer.
        while let Some(result) = work.join_next().await {
            if let Err(e) = join_result(result) {
                failure.get_or_insert(e);
            }
        }
        // Preserve network, identity and storage until terminal cleanup is confirmed.
        terminal_owner.lock().await.drain().await;
        if let Err(e) = services.network().await {
            failure.get_or_insert(e);
        }
        // No business producer remains. Drain already accepted persistence operations.
        persist_stop.send_replace(true);
        while let Some(result) = persistence.join_next().await {
            if let Err(e) = join_result(result) {
                failure.get_or_insert(e);
            }
        }
        if let Err(e) = services.remaining().await {
            failure.get_or_insert(e);
        }
        revision += 1;
        snapshots.send_replace(assemble(
            Inputs {
                configuration: &c.borrow(),
                devices: &d.borrow(),
                sync: &s.borrow(),
                history: &h.borrow(),
                network: &network.status(),
                startup: &startup,
                terminal: &terminal.snapshot(),
            },
            &session,
            revision,
            if failure.is_some() {
                AppState::Failed
            } else {
                AppState::Stopped
            },
        ));
        failure.map_or(Ok(()), Err)
    }
}
fn join_result(result: std::result::Result<Result<()>, tokio::task::JoinError>) -> Result<()> {
    result.map_err(|_| Error::Internal)?
}
fn task_error(result: Option<std::result::Result<Result<()>, tokio::task::JoinError>>) -> Error {
    result
        .and_then(|r| join_result(r).err())
        .unwrap_or(Error::Stopped)
}
