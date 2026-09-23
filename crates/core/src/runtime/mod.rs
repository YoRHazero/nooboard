//! Composition and supervision only; feature runtimes own their business state.
pub(crate) mod message;
pub(crate) mod routing;
pub(crate) mod shutdown;
pub(crate) mod startup;
use crate::{
    configuration::runtime as configuration,
    devices::runtime as devices,
    history::runtime as history,
    snapshot::assemble::{Inputs, assemble},
    sync::runtime as sync,
    *,
};
use nooboard_network::{Network, NetworkEvents};
use nooboard_storage::Storage;
use tokio::{
    sync::{broadcast, watch},
    task::JoinSet,
};
pub(crate) struct Running {
    services: shutdown::Services,
    stop: watch::Sender<bool>,
    stopped: watch::Receiver<bool>,
    persist_stop: watch::Sender<bool>,
    persist_stopped: watch::Receiver<bool>,
    config: configuration::Handle,
    config_runtime: configuration::Runtime,
    history: history::Handle,
    history_runtime: history::Runtime,
    device: devices::Channels,
    device_runtime: devices::Runtime,
    sync: sync::Channels,
    sync_runtime: sync::Runtime,
    storage: Storage,
    network: Network,
    network_events: NetworkEvents,
    snapshots: watch::Sender<AppSnapshot>,
    events: broadcast::Sender<Event>,
    session: String,
    startup: Settings,
}
impl Running {
    pub async fn run(self) -> Result<()> {
        let Self {
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
        persistence.spawn(config_runtime.run(storage.clone(), persist_stopped.clone()));
        persistence.spawn(history_runtime.run(storage, config.clone(), persist_stopped));
        work.spawn(device_runtime.run(stopped.clone()));
        work.spawn(sync_runtime.run(stopped.clone()));
        work.spawn(routing::run(
            network.clone(),
            network_events,
            device.events,
            sync.events,
            stopped.clone(),
        ));
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
             _=stopped.changed()=>break,
                result=work.join_next()=>{if !*stopped.borrow(){failure=Some(task_error(result));}break;},
             result=persistence.join_next()=>{failure=Some(task_error(result));break;},
             _=c.changed()=>{},_=h.changed()=>{},_=d.changed()=>{},_=s.changed()=>{},_=n.changed()=>{},
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
        // First finish business tasks while all three services can still answer.
        while let Some(result) = work.join_next().await {
            if let Err(e) = join_result(result) {
                failure.get_or_insert(e);
            }
        }
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
