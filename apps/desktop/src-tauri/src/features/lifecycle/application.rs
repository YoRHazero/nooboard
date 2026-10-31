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
    /// Keep exit admission closed throughout installation. Failure after shutdown
    /// resumes the existing application services before reporting the error.
    pub async fn restart_with(
        &self,
        install: impl std::future::Future<Output = crate::error::Result<()>>,
        restart: impl FnOnce(),
    ) -> crate::error::Result<()> {
        let ticket = self
            .control
            .begin(None)
            .ok_or(crate::error::Error::Closing)?;
        if let Err(error) = self.backend.shutdown().await {
            self.control.complete(ticket, false);
            return Err(error);
        }
        if let Err(error) = install.await {
            if self.control.complete(ticket, false) {
                self.backend.resume().await?;
            }
            return Err(error);
        }
        if self.control.complete(ticket, true) {
            restart();
        }
        Ok(())
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
        resumes: AtomicUsize,
        fail: AtomicBool,
        block: AtomicBool,
        entered: Notify,
        release: Notify,
    }
    impl BackendControl for Backend {
        fn resume(&self) -> Future<'_, ()> {
            Box::pin(async {
                self.resumes.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
        }
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
            resumes: AtomicUsize::new(0),
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
    #[tokio::test]
    async fn update_waits_for_shutdown_and_blocks_quit_until_install_finishes() {
        let (lifecycle, backend, ui, _) = fixture();
        backend.block.store(true, Ordering::SeqCst);
        let (installed, started) = oneshot::channel();
        let (finish, finished) = oneshot::channel();
        let task = tokio::spawn({
            let lifecycle = lifecycle.clone();
            let ui = ui.clone();
            async move {
                lifecycle
                    .restart_with(
                        async {
                            installed.send(()).unwrap();
                            finished.await.unwrap();
                            Ok(())
                        },
                        || {
                            ui.exits.fetch_add(1, Ordering::SeqCst);
                        },
                    )
                    .await
            }
        });
        bounded(backend.entered.notified()).await;
        assert_eq!(ui.exits.load(Ordering::SeqCst), 0);
        backend.release.notify_one();
        bounded(started).await.unwrap();
        assert_eq!(lifecycle.control.phase(), Phase::Closing);
        lifecycle.shutdown().await;
        assert!(matches!(
            lifecycle
                .restart_with(async { panic!("duplicate install") }, || {})
                .await,
            Err(Error::Closing)
        ));
        assert_eq!(backend.calls.load(Ordering::SeqCst), 1);
        assert_eq!(ui.exits.load(Ordering::SeqCst), 0);
        finish.send(()).unwrap();
        bounded(task).await.unwrap().unwrap();
        assert!(lifecycle.control.finished());
        assert_eq!(ui.exits.load(Ordering::SeqCst), 1);
    }
    #[tokio::test]
    async fn failed_shutdown_skips_install_and_failed_install_resumes_services() {
        let (lifecycle, backend, ui, _) = fixture();
        backend.fail.store(true, Ordering::SeqCst);
        assert!(
            lifecycle
                .restart_with(async { panic!("shutdown failed") }, || {})
                .await
                .is_err()
        );
        assert_eq!(lifecycle.control.phase(), Phase::Idle);
        assert_eq!(backend.resumes.load(Ordering::SeqCst), 0);
        let result = lifecycle
            .restart_with(async { Err(Error::UpdateInstall) }, || {
                panic!("install failed")
            })
            .await;
        assert!(matches!(result, Err(Error::UpdateInstall)));
        assert_eq!(backend.resumes.load(Ordering::SeqCst), 1);
        assert_eq!(lifecycle.control.phase(), Phase::Idle);
        assert_eq!(ui.exits.load(Ordering::SeqCst), 0);
    }
}
