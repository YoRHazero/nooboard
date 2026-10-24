use crate::{
    Settings,
    configuration::{domain::model::Configuration, ports::storage::ConfigurationRepository},
    contracts::PortFuture,
};
use std::sync::Arc;
struct ConfigurationAdapter(nooboard_storage::Storage);
pub(crate) fn port(storage: nooboard_storage::Storage) -> Arc<dyn ConfigurationRepository> {
    Arc::new(ConfigurationAdapter(storage))
}
impl ConfigurationRepository for ConfigurationAdapter {
    fn load(&self, defaults: Settings) -> PortFuture<'_, Configuration> {
        Box::pin(super::document::load(&self.0, defaults))
    }
    fn save(&self, configuration: Configuration) -> PortFuture<'_, ()> {
        Box::pin(async move { super::document::save(&self.0, &configuration).await })
    }
}
