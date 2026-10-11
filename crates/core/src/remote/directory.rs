use super::{model::*, runtime::Remote};
use crate::{Error, Result};
use nooboard_ssh::RemotePath;

impl Remote {
    pub async fn navigate(&self, target: RemoteFileTarget, input: String) -> Result<()> {
        self.check()?;
        let workspace = target.workspace;
        let (session, base, generation) = {
            let mut state = self.inner.lock().await;
            let w = state
                .view
                .workspaces
                .iter_mut()
                .find(|w| {
                    w.id == workspace
                        && !w.closed
                        && w.files_id == target.files
                        && w.generation == target.generation
                })
                .ok_or(Error::NotFound)?;
            w.navigation += 1;
            (
                w.files_id.clone(),
                w.root.clone().ok_or(Error::Offline)?,
                w.navigation,
            )
        };
        let path = self
            .ssh
            .validate_directory(session.clone(), base, input)
            .await?;
        let mut state = self.inner.lock().await;
        let w = state
            .view
            .workspaces
            .iter_mut()
            .find(|w| {
                w.id == workspace
                    && !w.closed
                    && w.files_id == session
                    && w.navigation == generation
            })
            .ok_or(nooboard_ssh::Error::Cancelled)?;
        let queries: Vec<_> = w
            .directories
            .iter()
            .filter_map(|d| d.query.clone())
            .collect();
        w.generation += 1;
        w.root = Some(path);
        std::sync::Arc::make_mut(&mut w.directories).clear();
        self.publish(&mut state);
        drop(state);
        for query in queries {
            let _ = self.ssh.cancel_directory(query).await;
        }
        Ok(())
    }
    /// Explicit expansion/refresh. No network work occurs merely because a root
    /// was validated or a row was rendered/hovered.
    pub async fn list_directory(
        &self,
        target: RemoteFileTarget,
        path: RemotePath,
        more: bool,
    ) -> Result<()> {
        self.check()?;
        let workspace = target.workspace;
        let (session, generation, query_generation, cursor, old_query) = {
            let mut state = self.inner.lock().await;
            let w = state
                .view
                .workspaces
                .iter_mut()
                .find(|w| {
                    w.id == workspace
                        && !w.closed
                        && w.files_id == target.files
                        && w.generation == target.generation
                })
                .ok_or(Error::NotFound)?;
            if w.directories.iter().map(|d| d.entries.len()).sum::<usize>() >= 20_000 {
                return Err(Error::Busy);
            }
            if let Some(d) = w.directories.iter().find(|d| d.path == path)
                && d.state == DirectoryState::Loading
            {
                return Err(Error::Busy);
            }
            let index = match w.directories.iter().position(|d| d.path == path) {
                Some(i) => i,
                None => {
                    if w.directories.len() >= 128 {
                        return Err(Error::Busy);
                    }
                    std::sync::Arc::make_mut(&mut w.directories).push(RemoteDirectory {
                        path: path.clone(),
                        entries: vec![],
                        state: DirectoryState::Stale,
                        error: None,
                        generation: 0,
                        cursor: None,
                        query: None,
                    });
                    w.directories.len() - 1
                }
            };
            let d = &mut std::sync::Arc::make_mut(&mut w.directories)[index];
            let cursor = if more {
                Some(d.cursor.take().ok_or(nooboard_ssh::Error::StaleCursor)?)
            } else {
                None
            };
            let old_query = if more { None } else { d.query.take() };
            d.generation += 1;
            d.state = DirectoryState::Loading;
            d.error = None;
            let values = (
                w.files_id.clone(),
                w.generation,
                d.generation,
                cursor,
                old_query,
            );
            self.publish(&mut state);
            values
        };
        if let Some(query) = old_query {
            let _ = self.ssh.cancel_directory(query).await;
        }
        let result = match cursor {
            Some(cursor) => self.ssh.next_directory_batch(cursor).await,
            None => self.ssh.read_directory(session.clone(), path.clone()).await,
        };
        let mut state = self.inner.lock().await;
        let directory = state
            .view
            .workspaces
            .iter_mut()
            .find(|w| {
                w.id == workspace
                    && !w.closed
                    && w.files_id == session
                    && w.generation == generation
            })
            .and_then(|w| {
                std::sync::Arc::make_mut(&mut w.directories)
                    .iter_mut()
                    .find(|d| d.path == path && d.generation == query_generation)
            });
        let Some(directory) = directory else {
            drop(state);
            if let Ok(batch) = result {
                let _ = self.ssh.cancel_directory(batch.query_id).await;
            }
            return Ok(());
        };
        let result = match result {
            Ok(batch) => {
                if !more {
                    directory.entries.clear();
                }
                directory.entries.extend(batch.entries);
                directory.cursor = batch.cursor;
                directory.query = (!batch.complete).then_some(batch.query_id);
                directory.state = if batch.complete {
                    DirectoryState::Complete
                } else {
                    DirectoryState::Partial
                };
                Ok(())
            }
            Err(error) => {
                directory.state = DirectoryState::Error;
                directory.error = Some(error.clone());
                Err(error.into())
            }
        };
        self.publish(&mut state);
        result
    }
    pub async fn collapse_directory(
        &self,
        target: RemoteFileTarget,
        path: RemotePath,
    ) -> Result<()> {
        self.check()?;
        let workspace = target.workspace;
        let mut state = self.inner.lock().await;
        let w = state
            .view
            .workspaces
            .iter_mut()
            .find(|w| {
                w.id == workspace
                    && !w.closed
                    && w.files_id == target.files
                    && w.generation == target.generation
            })
            .ok_or(Error::NotFound)?;
        if let Some(d) = std::sync::Arc::make_mut(&mut w.directories)
            .iter_mut()
            .find(|d| d.path == path)
        {
            d.generation += 1;
            if let Some(query) = d.query.take() {
                let _ = self.ssh.cancel_directory(query).await;
            }
            if d.state != DirectoryState::Complete {
                d.state = DirectoryState::Stale;
                d.cursor = None;
            }
        }
        self.publish(&mut state);
        Ok(())
    }
}
