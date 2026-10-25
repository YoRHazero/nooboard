use super::ports::{FilePicker, SelectedFiles, SelectedTransfers};
use crate::error::Result;
use std::sync::Arc;
pub struct FileSelection {
    picker: Arc<dyn FilePicker>,
}
impl FileSelection {
    pub fn new(picker: Arc<dyn FilePicker>) -> Self {
        Self { picker }
    }
    pub async fn upload(&self, target: &dyn SelectedTransfers, folder: bool) -> Result<()> {
        let selected = if folder {
            self.picker.directory().await?.map(|path| vec![path])
        } else {
            self.picker.files().await?
        };
        if let Some(paths) = selected {
            target.upload(paths).await?;
        }
        Ok(())
    }
    pub async fn download(&self, target: &dyn SelectedTransfers) -> Result<()> {
        if let Some(directory) = self.picker.directory().await? {
            target.download(directory).await?;
        }
        Ok(())
    }
    pub async fn send_files(&self, target: &dyn SelectedFiles) -> Result<()> {
        if let Some(paths) = self.picker.files().await? {
            target.send(paths).await?;
        }
        Ok(())
    }
    pub async fn receive_directory(&self, target: &dyn SelectedFiles) -> Result<()> {
        if let Some(path) = self.picker.directory().await? {
            target.set_receive_directory(path).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{Error, Future};
    use std::{path::PathBuf, sync::Mutex};
    struct Picker(bool);
    impl FilePicker for Picker {
        fn files(&self) -> Future<'_, Option<Vec<PathBuf>>> {
            Box::pin(async { Ok(self.0.then(|| vec![PathBuf::from("selected")])) })
        }
        fn directory(&self) -> Future<'_, Option<PathBuf>> {
            Box::pin(async { Ok(self.0.then(|| PathBuf::from("directory"))) })
        }
    }
    struct Target(Mutex<Vec<PathBuf>>);
    impl SelectedFiles for Target {
        fn send(&self, paths: Vec<PathBuf>) -> Future<'_, ()> {
            Box::pin(async move {
                self.0.lock().unwrap().extend(paths);
                Ok(())
            })
        }
        fn set_receive_directory(&self, _: PathBuf) -> Future<'_, ()> {
            Box::pin(async { Err(Error::Preferences) })
        }
    }
    #[tokio::test]
    async fn cancelling_a_picker_has_no_effect_and_apply_failures_reach_the_caller() {
        let target = Target(Mutex::new(vec![]));
        let cancelled = FileSelection::new(Arc::new(Picker(false)));
        cancelled.send_files(&target).await.unwrap();
        cancelled.receive_directory(&target).await.unwrap();
        assert!(target.0.lock().unwrap().is_empty());
        let selected = FileSelection::new(Arc::new(Picker(true)));
        selected.send_files(&target).await.unwrap();
        assert_eq!(*target.0.lock().unwrap(), vec![PathBuf::from("selected")]);
        assert!(selected.receive_directory(&target).await.is_err());
    }
}
