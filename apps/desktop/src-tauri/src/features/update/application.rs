use super::{
    domain::ReadyUpdate,
    ports::{DownloadedUpdate, InstallControl, UpdateSource},
};
use crate::error::{Error, Result};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{Mutex as AsyncMutex, watch};

pub struct Updater {
    source: Arc<dyn UpdateSource>,
    installer: Arc<dyn InstallControl>,
    operation: AsyncMutex<()>,
    pending: Mutex<Option<Box<dyn DownloadedUpdate>>>,
    state: watch::Sender<Option<ReadyUpdate>>,
}
impl Updater {
    pub fn new(source: Arc<dyn UpdateSource>, installer: Arc<dyn InstallControl>) -> Self {
        Self {
            source,
            installer,
            operation: AsyncMutex::new(()),
            pending: Mutex::new(None),
            state: watch::channel(None).0,
        }
    }
    pub fn snapshot(&self) -> Option<ReadyUpdate> {
        self.state.borrow().clone()
    }
    pub fn subscribe(&self) -> watch::Receiver<Option<ReadyUpdate>> {
        self.state.subscribe()
    }
    pub async fn run(&self) {
        loop {
            self.check_once().await;
            tokio::time::sleep(Duration::from_secs(6 * 60 * 60)).await;
        }
    }
    async fn check_once(&self) {
        let Ok(_operation) = self.operation.try_lock() else {
            return;
        };
        if self.pending.lock().unwrap().is_some() {
            return;
        }
        // Missing releases, network failures and rejected signatures stay invisible.
        if let Ok(Some(update)) = self.source.download().await {
            let state = ReadyUpdate {
                version: update.version().into(),
                installing: false,
            };
            *self.pending.lock().unwrap() = Some(update);
            self.state.send_replace(Some(state));
        }
    }
    pub async fn install(&self) -> Result<()> {
        let _operation = self.operation.try_lock().map_err(|_| Error::Closing)?;
        let update = self
            .pending
            .lock()
            .unwrap()
            .take()
            .ok_or(Error::UpdateUnavailable)?;
        self.state.send_replace(Some(ReadyUpdate {
            version: update.version().into(),
            installing: true,
        }));
        let result = self.installer.install_and_restart(&*update).await;
        if result.is_err() {
            let state = ReadyUpdate {
                version: update.version().into(),
                installing: false,
            };
            *self.pending.lock().unwrap() = Some(update);
            self.state.send_replace(Some(state));
        } else {
            self.state.send_replace(None);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Future;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use tokio::sync::Notify;

    #[derive(Default)]
    struct Source {
        calls: AtomicUsize,
        fail: AtomicBool,
        block: AtomicBool,
        entered: Notify,
        release: Notify,
    }
    struct Package;
    impl DownloadedUpdate for Package {
        fn version(&self) -> &str {
            "1.2.3"
        }
        fn install(&self) -> Future<'_, ()> {
            Box::pin(async { Ok(()) })
        }
    }
    impl UpdateSource for Source {
        fn download(&self) -> Future<'_, Option<Box<dyn DownloadedUpdate>>> {
            Box::pin(async {
                self.calls.fetch_add(1, Ordering::SeqCst);
                self.entered.notify_one();
                if self.block.load(Ordering::SeqCst) {
                    self.release.notified().await;
                }
                if self.fail.swap(false, Ordering::SeqCst) {
                    return Err(Error::UpdateUnavailable);
                }
                Ok(Some(Box::new(Package) as Box<dyn DownloadedUpdate>))
            })
        }
    }
    #[derive(Default)]
    struct Installer {
        calls: AtomicUsize,
        fail: AtomicBool,
        block: AtomicBool,
        entered: Notify,
        release: Notify,
    }
    impl InstallControl for Installer {
        fn install_and_restart<'a>(&'a self, update: &'a dyn DownloadedUpdate) -> Future<'a, ()> {
            Box::pin(async move {
                self.calls.fetch_add(1, Ordering::SeqCst);
                self.entered.notify_one();
                if self.block.load(Ordering::SeqCst) {
                    self.release.notified().await;
                }
                if self.fail.swap(false, Ordering::SeqCst) {
                    return Err(Error::UpdateInstall);
                }
                update.install().await
            })
        }
    }
    fn fixture() -> (Arc<Updater>, Arc<Source>, Arc<Installer>) {
        let source = Arc::new(Source::default());
        let installer = Arc::new(Installer::default());
        (
            Arc::new(Updater::new(source.clone(), installer.clone())),
            source,
            installer,
        )
    }
    async fn bounded<T>(future: impl std::future::Future<Output = T>) -> T {
        tokio::time::timeout(Duration::from_secs(3), future)
            .await
            .unwrap()
    }
    #[tokio::test]
    async fn failed_checks_stay_silent_and_retry_without_downloading_a_ready_update_again() {
        let (updater, source, _) = fixture();
        let updates = updater.subscribe();
        source.fail.store(true, Ordering::SeqCst);
        updater.check_once().await;
        assert!(!updates.has_changed().unwrap());
        assert_eq!(updater.snapshot(), None);
        updater.check_once().await;
        assert_eq!(
            updater.snapshot(),
            Some(ReadyUpdate {
                version: "1.2.3".into(),
                installing: false
            })
        );
        updater.check_once().await;
        assert_eq!(source.calls.load(Ordering::SeqCst), 2);
    }
    #[tokio::test]
    async fn no_notice_or_install_before_verified_download_finishes() {
        let (updater, source, installer) = fixture();
        assert!(matches!(
            updater.install().await,
            Err(Error::UpdateUnavailable)
        ));
        source.block.store(true, Ordering::SeqCst);
        let check = tokio::spawn({
            let updater = updater.clone();
            async move { updater.check_once().await }
        });
        bounded(source.entered.notified()).await;
        assert_eq!(updater.snapshot(), None);
        assert!(updater.install().await.is_err());
        updater.check_once().await;
        assert_eq!(source.calls.load(Ordering::SeqCst), 1);
        assert_eq!(installer.calls.load(Ordering::SeqCst), 0);
        source.release.notify_one();
        bounded(check).await.unwrap();
        assert!(updater.snapshot().is_some());
    }
    #[tokio::test]
    async fn duplicate_installs_are_rejected_and_failure_keeps_verified_package_for_retry() {
        let (updater, source, installer) = fixture();
        updater.check_once().await;
        installer.fail.store(true, Ordering::SeqCst);
        installer.block.store(true, Ordering::SeqCst);
        let install = tokio::spawn({
            let updater = updater.clone();
            async move { updater.install().await }
        });
        bounded(installer.entered.notified()).await;
        assert!(updater.snapshot().unwrap().installing);
        assert!(matches!(updater.install().await, Err(Error::Closing)));
        installer.release.notify_one();
        assert!(matches!(
            bounded(install).await.unwrap(),
            Err(Error::UpdateInstall)
        ));
        assert!(!updater.snapshot().unwrap().installing);
        installer.block.store(false, Ordering::SeqCst);
        updater.install().await.unwrap();
        assert_eq!(updater.snapshot(), None);
        assert_eq!(source.calls.load(Ordering::SeqCst), 1);
        assert_eq!(installer.calls.load(Ordering::SeqCst), 2);
    }
}
