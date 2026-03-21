use gpui::{Context, Entity, Task};
use nooboard_core::{IncomingTransferDecision, SendFilesRequest, TransferTicket};

use crate::workspace::controller::WorkspaceController;

use super::spawn::spawn_core_call;

pub fn send_files_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    request: SendFilesRequest,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<Vec<TransferTicket>>>> {
    spawn_core_call(
        controller,
        cx,
        "couldn't send files",
        move |core| async move { core.send_files(request).await },
    )
}

pub fn decide_incoming_transfer_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    decision: IncomingTransferDecision,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<()>>> {
    spawn_core_call(
        controller,
        cx,
        "couldn't respond to a transfer request",
        move |core| async move { core.decide_incoming_transfer(decision).await },
    )
}

pub fn cancel_transfer_task<T: 'static>(
    controller: &Entity<WorkspaceController>,
    ticket: TransferTicket,
    cx: &Context<T>,
) -> Option<Task<nooboard_core::CoreResult<()>>> {
    spawn_core_call(
        controller,
        cx,
        "couldn't cancel a transfer",
        move |core| async move { core.cancel_transfer(ticket).await },
    )
}
