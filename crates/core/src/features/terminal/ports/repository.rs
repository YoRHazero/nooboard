use crate::{contracts::PortFuture, features::terminal::domain::config::Catalog};
pub(crate) trait TerminalRepository: Send + Sync {
    fn load(&self) -> PortFuture<'_, Catalog>;
    fn save(&self, catalog: Catalog) -> PortFuture<'_, ()>;
}
