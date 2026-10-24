use crate::{Settings, configuration::domain::model::Configuration, contracts::PortFuture};
pub(crate) trait ConfigurationRepository: Send + Sync {
    fn load(&self, defaults: Settings) -> PortFuture<'_, Configuration>;
    fn save(&self, configuration: Configuration) -> PortFuture<'_, ()>;
}
