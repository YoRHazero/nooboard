use crate::{
    configuration::{
        application::runtime::Handle,
        domain::{change::Change, model::Configuration},
        ports::settings::ConfigurationPort,
    },
    contracts::PortFuture,
};
use tokio::sync::watch;
impl ConfigurationPort for Handle {
    fn current(&self) -> Configuration {
        self.current()
    }
    fn subscribe(&self) -> watch::Receiver<Configuration> {
        self.state.clone()
    }
    fn change(&self, change: Change) -> PortFuture<'_, ()> {
        Box::pin(self.change(change))
    }
}
