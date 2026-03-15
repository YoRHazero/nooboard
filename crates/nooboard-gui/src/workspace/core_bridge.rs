use std::sync::Arc;

use nooboard_core::{
    BootstrapLaunch, ClipboardRecord, CoreResult, NooboardCore, WorkspaceSnapshot,
};

use super::subscriptions::WorkspaceSubscriptions;

#[derive(Clone)]
pub struct CoreBridge {
    core: Arc<NooboardCore>,
}

pub struct CoreBridgeBoot {
    pub bridge: CoreBridge,
    pub snapshot: WorkspaceSnapshot,
    pub latest_committed_record: Option<ClipboardRecord>,
    pub subscriptions: WorkspaceSubscriptions,
}

impl CoreBridge {
    pub async fn launch(launch: &BootstrapLaunch) -> CoreResult<CoreBridgeBoot> {
        let core = Arc::new(NooboardCore::launch_default(launch)?);
        let snapshot = core.snapshot().await?;
        let latest_committed_record = fetch_latest_committed_record(core.as_ref(), &snapshot).await?;
        let subscriptions = WorkspaceSubscriptions {
            state: core.subscribe_state().await?,
            events: core.subscribe_events().await?,
        };

        Ok(CoreBridgeBoot {
            bridge: Self { core },
            snapshot,
            latest_committed_record,
            subscriptions,
        })
    }

    pub fn core(&self) -> Arc<NooboardCore> {
        self.core.clone()
    }

    pub async fn fetch_latest_committed_record(
        &self,
        snapshot: &WorkspaceSnapshot,
    ) -> CoreResult<Option<ClipboardRecord>> {
        fetch_latest_committed_record(self.core.as_ref(), snapshot).await
    }
}

async fn fetch_latest_committed_record(
    core: &NooboardCore,
    snapshot: &WorkspaceSnapshot,
) -> CoreResult<Option<ClipboardRecord>> {
    match snapshot.clipboard.latest_committed_event_id {
        Some(event_id) => core.get_clipboard_record(event_id).await.map(Some),
        None => Ok(None),
    }
}
