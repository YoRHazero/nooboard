use super::{
    domain::{ExitControl, Ticket},
    ports::{BackendControl, ExitInteraction},
};
use std::sync::Arc;
pub struct Lifecycle {
    pub control: Arc<ExitControl>,
    backend: Arc<dyn BackendControl>,
    interaction: Arc<dyn ExitInteraction>,
    confirmation_supported: bool,
}
impl Lifecycle {
    pub fn new(
        control: Arc<ExitControl>,
        backend: Arc<dyn BackendControl>,
        interaction: Arc<dyn ExitInteraction>,
        confirmation_supported: bool,
    ) -> Self {
        Self {
            control,
            backend,
            interaction,
            confirmation_supported,
        }
    }
    pub async fn request_quit(&self) {
        let Some(ticket) = self.control.ask() else {
            return;
        };
        let count = self.backend.pending_transfers().await;
        if !self.control.is_asking(ticket) {
            return;
        }
        if count > 0 && self.confirmation_supported && !self.interaction.confirm(count).await {
            self.control.cancel(ticket);
            return;
        }
        self.close(Some(ticket)).await;
    }
    /// OS exit skips confirmation and invalidates any pending confirmation result.
    pub async fn shutdown(&self) {
        self.close(None).await;
    }
    async fn close(&self, confirmation: Option<Ticket>) {
        let Some(ticket) = self.control.begin(confirmation) else {
            return;
        };
        let success = self.backend.shutdown().await.is_ok();
        if !self.control.complete(ticket, success) {
            return;
        }
        if success {
            self.interaction.exit();
        } else {
            self.interaction.failed().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        error::{Error, Future},
        features::lifecycle::{domain::Phase, ports::Interaction},
    };
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use tokio::sync::{Notify, mpsc, oneshot};
    struct Backend {
        calls: AtomicUsize,
        fail: AtomicBool,
        block: AtomicBool,
        entered: Notify,
        release: Notify,
    }
    impl BackendControl for Backend {
        fn pending_transfers(&self) -> Interaction<'_, usize> {
            Box::pin(async { 1 })
        }
        fn shutdown(&self) -> Future<'_, ()> {
            Box::pin(async {
                self.calls.fetch_add(1, Ordering::SeqCst);
                self.entered.notify_one();
                if self.block.load(Ordering::SeqCst) {
                    self.release.notified().await;
                }
                if self.fail.swap(false, Ordering::SeqCst) {
                    Err(Error::Closing)
                } else {
                    Ok(())
                }
            })
        }
    }
    struct Ui {
        requests: mpsc::UnboundedSender<oneshot::Sender<bool>>,
        failures: AtomicUsize,
        exits: AtomicUsize,
    }
    impl ExitInteraction for Ui {
        fn confirm(&self, _: usize) -> Interaction<'_, bool> {
            Box::pin(async {
                let (tx, rx) = oneshot::channel();
                self.requests.send(tx).unwrap();
                rx.await.unwrap()
            })
        }
        fn failed(&self) -> Interaction<'_, ()> {
            Box::pin(async {
                self.failures.fetch_add(1, Ordering::SeqCst);
            })
        }
        fn exit(&self) {
            self.exits.fetch_add(1, Ordering::SeqCst);
        }
    }
    fn fixture() -> (
        Arc<Lifecycle>,
        Arc<Backend>,
        Arc<Ui>,
        mpsc::UnboundedReceiver<oneshot::Sender<bool>>,
    ) {
        let (tx, rx) = mpsc::unbounded_channel();
        let backend = Arc::new(Backend {
            calls: AtomicUsize::new(0),
            fail: AtomicBool::new(false),
            block: AtomicBool::new(false),
            entered: Notify::new(),
            release: Notify::new(),
        });
        let ui = Arc::new(Ui {
            requests: tx,
            failures: AtomicUsize::new(0),
            exits: AtomicUsize::new(0),
        });
        (
            Arc::new(Lifecycle::new(
                Arc::new(ExitControl::default()),
                backend.clone(),
                ui.clone(),
                true,
            )),
            backend,
            ui,
            rx,
        )
    }
    async fn bounded<T>(value: impl std::future::Future<Output = T>) -> T {
        tokio::time::timeout(std::time::Duration::from_secs(3), value)
            .await
            .unwrap()
    }
    #[tokio::test]
    async fn late_confirmation_cannot_resume_or_cancel_a_new_quit_after_failed_shutdown() {
        let (lifecycle, backend, ui, mut prompts) = fixture();
        let first = tokio::spawn({
            let lifecycle = lifecycle.clone();
            async move {
                lifecycle.request_quit().await;
            }
        });
        let stale = bounded(prompts.recv()).await.unwrap();
        lifecycle.request_quit().await; // Duplicate asking request is coalesced.
        backend.fail.store(true, Ordering::SeqCst);
        bounded(lifecycle.shutdown()).await;
        assert_eq!(lifecycle.control.phase(), Phase::Idle);
        assert_eq!(ui.failures.load(Ordering::SeqCst), 1);
        let second = tokio::spawn({
            let lifecycle = lifecycle.clone();
            async move {
                lifecycle.request_quit().await;
            }
        });
        let current = bounded(prompts.recv()).await.unwrap();
        stale.send(true).unwrap();
        bounded(first).await.unwrap();
        assert_eq!(lifecycle.control.phase(), Phase::Asking);
        assert_eq!(backend.calls.load(Ordering::SeqCst), 1);
        current.send(false).unwrap();
        bounded(second).await.unwrap();
        assert_eq!(lifecycle.control.phase(), Phase::Idle);
        bounded(lifecycle.shutdown()).await;
        assert_eq!(backend.calls.load(Ordering::SeqCst), 2);
        assert_eq!(ui.exits.load(Ordering::SeqCst), 1);
    }
    #[tokio::test]
    async fn repeated_exit_requests_wait_for_cleanup_and_never_exit_early() {
        let (lifecycle, backend, ui, _) = fixture();
        backend.block.store(true, Ordering::SeqCst);
        let closing = tokio::spawn({
            let lifecycle = lifecycle.clone();
            async move {
                lifecycle.shutdown().await;
            }
        });
        bounded(backend.entered.notified()).await;
        bounded(lifecycle.shutdown()).await;
        bounded(lifecycle.request_quit()).await;
        assert_eq!(backend.calls.load(Ordering::SeqCst), 1);
        assert_eq!(ui.exits.load(Ordering::SeqCst), 0);
        assert_eq!(lifecycle.control.phase(), Phase::Closing);
        backend.release.notify_one();
        bounded(closing).await.unwrap();
        assert!(lifecycle.control.finished());
        assert_eq!(ui.exits.load(Ordering::SeqCst), 1);
    }
}
