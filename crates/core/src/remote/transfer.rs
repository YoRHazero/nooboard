use super::{
    model::{ConfigurationChange, RemoteFileTarget},
    runtime::Remote,
};
use crate::{Error, Result, configuration::runtime::Change};
use nooboard_ssh::{RemotePath, TransferSpec};
use std::path::PathBuf;
impl Remote {
    pub async fn set_download_directory(&self, path: PathBuf) -> Result<()> {
        self.check()?;
        if !path.is_absolute()
            || !tokio::fs::metadata(&path)
                .await
                .map_err(nooboard_ssh::Error::from)?
                .is_dir()
        {
            return Err(nooboard_ssh::Error::NotDirectory.into());
        }
        self.configuration
            .change(Change::Remote(ConfigurationChange::DownloadDirectory(path)))
            .await?;
        let mut state = self.inner.lock().await;
        self.publish(&mut state);
        Ok(())
    }
    async fn file_session(&self, target: &RemoteFileTarget) -> Result<String> {
        self.check()?;
        self.inner
            .lock()
            .await
            .view
            .workspaces
            .iter()
            .find(|w| w.id == target.workspace && !w.closed && w.files_id == target.files)
            .map(|w| w.files_id.clone())
            .ok_or(Error::NotFound)
    }
    pub async fn upload(
        &self,
        target: RemoteFileTarget,
        directory: RemotePath,
        paths: Vec<PathBuf>,
    ) -> Result<()> {
        if paths.is_empty() || paths.len() > 100 {
            return Err(Error::Configuration);
        }
        let session = self.file_session(&target).await?;
        // Validate against SFTP, without expanding the destination directory.
        let directory = self
            .ssh
            .validate_directory(session.clone(), directory, ".".into())
            .await?;
        for local in paths {
            self.ssh
                .transfer(
                    session.clone(),
                    TransferSpec::Upload {
                        local,
                        remote_directory: directory.clone(),
                    },
                )
                .await?;
        }
        Ok(())
    }
    pub async fn download(&self, target: RemoteFileTarget, paths: Vec<RemotePath>) -> Result<()> {
        if paths.is_empty() || paths.len() > 100 {
            return Err(Error::Configuration);
        }
        let session = self.file_session(&target).await?;
        let local_directory = self
            .configuration
            .current()
            .remote
            .download_directory
            .ok_or(Error::Configuration)?;
        for remote in paths {
            self.ssh
                .transfer(
                    session.clone(),
                    TransferSpec::Download {
                        remote,
                        local_directory: local_directory.clone(),
                    },
                )
                .await?;
        }
        Ok(())
    }
    pub async fn cancel_transfer(&self, id: String) -> Result<()> {
        self.check()?;
        Ok(self.ssh.cancel_transfer(id).await?)
    }
    pub async fn retry_transfer(&self, id: String) -> Result<()> {
        self.check()?;
        let task = self
            .ssh
            .snapshot()
            .transfers
            .into_iter()
            .find(|t| t.id == id)
            .ok_or(Error::NotFound)?;
        if !matches!(
            task.state,
            nooboard_ssh::TransferState::Failed | nooboard_ssh::TransferState::Cancelled
        ) {
            return Err(Error::Configuration);
        }
        // Re-authentication may replace only the file connection. Retry preserves
        // the original device, source and destination rather than the current UI.
        let session = {
            let state = self.inner.lock().await;
            let profile = state
                .attempt_profiles
                .get(&task.session_id)
                .ok_or(Error::Offline)?;
            state
                .view
                .workspaces
                .iter()
                .find(|w| {
                    !w.closed
                        && &w.profile_id == profile
                        && w.files == nooboard_ssh::ConnectionState::Ready
                })
                .map(|w| w.files_id.clone())
                .ok_or(Error::Offline)?
        };
        self.ssh.transfer(session, task.spec).await?;
        Ok(())
    }
    pub async fn forget_transfer(&self, id: String) -> Result<()> {
        self.check()?;
        Ok(self.ssh.forget_transfer(id).await?)
    }
    /// Only completed downloads are eligible for native reveal.
    pub fn downloaded_file(&self, id: &str) -> Result<PathBuf> {
        self.check()?;
        self.ssh
            .snapshot()
            .transfers
            .iter()
            .find(|t| {
                t.id == id
                    && t.state == nooboard_ssh::TransferState::Succeeded
                    && matches!(t.spec, TransferSpec::Download { .. })
            })
            .and_then(|t| t.target.as_ref())
            .map(PathBuf::from)
            .ok_or(Error::NotFound)
    }
}
