//! Shared value contracts only. Executable service handles are deliberately excluded.
pub(crate) mod clipboard;
pub(crate) mod network;
pub(crate) mod storage;
pub(crate) type PortFuture<'a, T> =
    std::pin::Pin<Box<dyn std::future::Future<Output = crate::Result<T>> + Send + 'a>>;
pub(crate) mod terminal;
