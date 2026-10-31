use super::ports::FilePicker;
use crate::error::Future;
use std::{path::PathBuf, sync::Mutex};
pub struct NativeFilePicker {
    history_path: Option<PathBuf>,
    last_download: Mutex<Option<PathBuf>>,
}
impl NativeFilePicker {
    pub fn new(history_path: Option<PathBuf>) -> Self {
        let last_download = history_path
            .as_ref()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<PathBuf>(&bytes).ok());
        Self {
            history_path,
            last_download: Mutex::new(last_download),
        }
    }
    fn remember_download(&self, directory: PathBuf) {
        *self.last_download.lock().unwrap_or_else(|p| p.into_inner()) = Some(directory.clone());
        if let Some(path) = &self.history_path {
            // Remembering a chooser preference must not fail an otherwise valid transfer.
            if let (Some(parent), Ok(bytes)) = (path.parent(), serde_json::to_vec(&directory)) {
                let _ = std::fs::create_dir_all(parent);
                let temporary = path.with_extension("tmp");
                if std::fs::write(&temporary, bytes).is_ok() {
                    let _ = std::fs::rename(temporary, path);
                }
            }
        }
    }
}
impl FilePicker for NativeFilePicker {
    fn files(&self) -> Future<'_, Option<Vec<PathBuf>>> {
        Box::pin(async {
            Ok(rfd::AsyncFileDialog::new()
                .pick_files()
                .await
                .map(|files| files.iter().map(|f| f.path().to_owned()).collect()))
        })
    }
    fn directory(&self) -> Future<'_, Option<PathBuf>> {
        Box::pin(async {
            Ok(rfd::AsyncFileDialog::new()
                .pick_folder()
                .await
                .map(|f| f.path().to_owned()))
        })
    }
    fn download_directory(&self) -> Future<'_, Option<PathBuf>> {
        Box::pin(async {
            let previous = self
                .last_download
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .clone();
            let mut picker = rfd::AsyncFileDialog::new();
            if let Some(directory) = previous.filter(|path| path.is_dir()) {
                picker = picker.set_directory(directory);
            }
            let selected = picker
                .pick_folder()
                .await
                .map(|file| file.path().to_owned());
            if let Some(directory) = &selected {
                self.remember_download(directory.clone());
            }
            Ok(selected)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn download_destination_survives_restart_and_corrupt_history_is_ignored() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = temporary.path().to_owned();
        let history = directory.join("download.json");
        let picker = NativeFilePicker::new(Some(history.clone()));
        picker.remember_download(directory.clone());
        let restored = NativeFilePicker::new(Some(history.clone()));
        assert_eq!(
            *restored.last_download.lock().unwrap(),
            Some(directory.clone())
        );
        std::fs::write(&history, "invalid").unwrap();
        assert!(
            NativeFilePicker::new(Some(history))
                .last_download
                .lock()
                .unwrap()
                .is_none()
        );
    }
}
