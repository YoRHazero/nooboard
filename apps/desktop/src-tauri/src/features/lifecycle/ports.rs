use crate::error::Future;
use std::{future::Future as StdFuture, pin::Pin};
pub type Interaction<'a, T> = Pin<Box<dyn StdFuture<Output = T> + Send + 'a>>;
pub trait BackendControl: Send + Sync {
    fn pending_transfers(&self) -> Interaction<'_, usize>;
    fn shutdown(&self) -> Future<'_, ()>;
}
pub trait ExitInteraction: Send + Sync {
    fn confirm(&self, count: usize) -> Interaction<'_, bool>;
    fn failed(&self) -> Interaction<'_, ()>;
    fn exit(&self);
}
