use super::ports::FilePicker;
use crate::error::Future;
use std::path::PathBuf;
pub struct NativeFilePicker;
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
}
