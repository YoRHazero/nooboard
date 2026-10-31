use crate::error::Future;

pub trait DownloadedUpdate: Send + Sync {
    fn version(&self) -> &str;
    fn install(&self) -> Future<'_, ()>;
}
pub trait UpdateSource: Send + Sync {
    /// Returns only after both download and signature verification succeed.
    fn download(&self) -> Future<'_, Option<Box<dyn DownloadedUpdate>>>;
}
pub trait InstallControl: Send + Sync {
    fn install_and_restart<'a>(&'a self, update: &'a dyn DownloadedUpdate) -> Future<'a, ()>;
}
