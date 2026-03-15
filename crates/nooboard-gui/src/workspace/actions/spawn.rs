use std::future::Future;

use gpui::{Context, Entity, Task};
use nooboard_core::NooboardCore;

use crate::workspace::controller::WorkspaceController;

pub(super) fn spawn_core_call<T: 'static, R: 'static, Fut>(
    controller: &Entity<WorkspaceController>,
    cx: &Context<T>,
    error_prefix: &'static str,
    op: impl FnOnce(std::sync::Arc<NooboardCore>) -> Fut + 'static,
) -> Option<Task<nooboard_core::CoreResult<R>>>
where
    Fut: Future<Output = nooboard_core::CoreResult<R>> + 'static,
{
    let Some(core) = controller.read(cx).core() else {
        return None;
    };
    let controller = controller.downgrade();

    Some(cx.spawn(async move |_, cx| {
        let result = op(core).await;
        if let Err(error) = &result {
            let _ = controller.update(cx, |this, cx| {
                this.record_bridge_warning(format!("{error_prefix}: {error}"));
                cx.notify();
            });
        }
        result
    }))
}
