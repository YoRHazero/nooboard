use super::ports::{DownloadedUpdate, UpdateSource};
use crate::error::{Error, Future};
use std::{sync::Arc, time::Duration};
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

pub struct NativeUpdateSource(pub AppHandle);
struct VerifiedUpdate {
    update: tauri_plugin_updater::Update,
    bytes: Arc<Vec<u8>>,
}
impl UpdateSource for NativeUpdateSource {
    fn download(&self) -> Future<'_, Option<Box<dyn DownloadedUpdate>>> {
        Box::pin(async move {
            // The Linux installer supported by Tauri's updater is AppImage.
            #[cfg(target_os = "linux")]
            if std::env::var_os("APPIMAGE").is_none() {
                return Ok(None);
            }
            let updater = self
                .0
                .updater_builder()
                .timeout(Duration::from_secs(30))
                .version_comparator(|current, release| {
                    release.version > current && release.version.pre.is_empty()
                })
                .build()
                .map_err(|_| Error::UpdateUnavailable)?;
            let Some(mut update) = updater
                .check()
                .await
                .map_err(|_| Error::UpdateUnavailable)?
            else {
                return Ok(None);
            };
            update.timeout = Some(Duration::from_secs(10 * 60));
            // download() verifies the fixed release public key before returning bytes.
            let bytes = update
                .download(|_, _| {}, || {})
                .await
                .map_err(|_| Error::UpdateUnavailable)?;
            Ok(Some(Box::new(VerifiedUpdate {
                update,
                bytes: Arc::new(bytes),
            }) as Box<dyn DownloadedUpdate>))
        })
    }
}
impl DownloadedUpdate for VerifiedUpdate {
    fn version(&self) -> &str {
        &self.update.version
    }
    fn install(&self) -> Future<'_, ()> {
        let update = self.update.clone();
        let bytes = self.bytes.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || update.install(bytes.as_slice()))
                .await
                .map_err(|_| Error::UpdateInstall)?
                .map_err(|_| Error::UpdateInstall)
        })
    }
}
