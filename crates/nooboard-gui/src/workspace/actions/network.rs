use gpui::{Context, Entity, Task};
use nooboard_core::{
    ConnectDirectOutcome, DirectRequestId, DirectSeedId, DirectSeedInfo, SessionId,
    UpsertDirectSeedInput,
};

use crate::workspace::controller::WorkspaceController;

use super::spawn::spawn_core_call;

pub fn start_network<T: 'static>(controller: &Entity<WorkspaceController>, cx: &Context<T>) {
    if let Some(task) = start_network_task(controller, cx) {
        task.detach();
    }
}

pub fn start_network_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<()>>> {
    spawn_core_call(
        controller,
        cx,
        "couldn't start network sharing",
        |core| async move { core.start_network().await },
    )
}

pub fn stop_network<T: 'static>(controller: &Entity<WorkspaceController>, cx: &Context<T>) {
    if let Some(task) = stop_network_task(controller, cx) {
        task.detach();
    }
}

pub fn stop_network_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<()>>> {
    spawn_core_call(
        controller,
        cx,
        "couldn't stop network sharing",
        |core| async move { core.stop_network().await },
    )
}

pub fn upsert_direct_seed_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    input: UpsertDirectSeedInput,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<DirectSeedId>>> {
    spawn_core_call(
        controller,
        cx,
        "couldn't save a device",
        move |core| async move { core.upsert_direct_seed(input).await },
    )
}

pub fn remove_direct_seed_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    id: DirectSeedId,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<()>>> {
    spawn_core_call(
        controller,
        cx,
        "couldn't remove a device",
        move |core| async move { core.remove_direct_seed(id).await },
    )
}

pub fn connect_direct_seed_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    id: DirectSeedId,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<ConnectDirectOutcome>>> {
    spawn_core_call(
        controller,
        cx,
        "couldn't connect to a device",
        move |core| async move { core.connect_direct_seed(id).await },
    )
}

pub fn search_direct_seeds_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    query: String,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<Vec<DirectSeedInfo>>>> {
    spawn_core_call(
        controller,
        cx,
        "couldn't search saved devices",
        move |core| async move { core.search_direct_seeds(&query).await },
    )
}

pub fn approve_direct_request_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    id: DirectRequestId,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<()>>> {
    spawn_core_call(
        controller,
        cx,
        "couldn't approve a connection request",
        move |core| async move { core.approve_direct_request(id).await },
    )
}

pub fn reject_direct_request_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    id: DirectRequestId,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<()>>> {
    spawn_core_call(
        controller,
        cx,
        "couldn't reject a connection request",
        move |core| async move { core.reject_direct_request(id).await },
    )
}

pub fn disconnect_session_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    id: SessionId,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<()>>> {
    spawn_core_call(
        controller,
        cx,
        "couldn't disconnect from a device",
        move |core| async move { core.disconnect_session(id).await },
    )
}
