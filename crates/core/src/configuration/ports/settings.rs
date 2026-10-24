use crate::{
    configuration::domain::{change::Change, model::Configuration},
    contracts::PortFuture,
};
use tokio::sync::watch;
pub(crate) trait ConfigurationPort: Send + Sync {
    fn current(&self) -> Configuration;
    fn subscribe(&self) -> watch::Receiver<Configuration>;
    fn change(&self, change: Change) -> PortFuture<'_, ()>;
}
