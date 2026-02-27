use std::sync::Arc;

use tokio::sync::broadcast;

use crate::AppResult;

use super::{AppEvent, AppServiceImpl};

impl AppServiceImpl {
    pub(super) async fn subscribe_events_usecase(
        &self,
    ) -> AppResult<broadcast::Receiver<AppEvent>> {
        self.subscriptions
            .subscribe(Arc::clone(&self.sync_runtime))
            .await
    }
}
