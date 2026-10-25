use crate::error::Future;
use std::path::PathBuf;
pub trait FilePicker: Send + Sync {
    fn files(&self) -> Future<'_, Option<Vec<PathBuf>>>;
    fn directory(&self) -> Future<'_, Option<PathBuf>>;
}
pub trait SelectedFiles: Send + Sync {
    fn send(&self, paths: Vec<PathBuf>) -> Future<'_, ()>;
    fn set_receive_directory(&self, path: PathBuf) -> Future<'_, ()>;
}

pub trait SelectedTransfers: Send + Sync {
    fn upload(&self, paths: Vec<PathBuf>) -> Future<'_, ()>;
    fn download(&self, directory: PathBuf) -> Future<'_, ()>;
}
