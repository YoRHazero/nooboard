use crate::AppResult;

use super::{AppServiceImpl, EventSubscription};

impl AppServiceImpl {
    pub(super) async fn subscribe_events_usecase(&self) -> AppResult<EventSubscription> {
        self.subscriptions.subscribe().await
    }
}
