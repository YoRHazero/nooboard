use crate::{
    error::Future,
    features::{
        lifecycle::application::Lifecycle,
        update::ports::{DownloadedUpdate, InstallControl},
    },
};
use std::sync::Arc;
use tauri::AppHandle;

pub struct NativeInstall {
    pub lifecycle: Arc<Lifecycle>,
    pub handle: AppHandle,
}
impl InstallControl for NativeInstall {
    fn install_and_restart<'a>(&'a self, update: &'a dyn DownloadedUpdate) -> Future<'a, ()> {
        Box::pin(async move {
            self.lifecycle
                .restart_with(update.install(), || self.handle.restart())
                .await
        })
    }
}
