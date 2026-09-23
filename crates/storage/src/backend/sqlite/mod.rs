mod connection;
mod error;
mod history;
mod migration;
mod settings;
use super::contract::{Backend, BackendFuture};
use crate::{
    Error, ErrorKind, HistoryEntry, HistoryId, HistoryPage, HistoryQuery, RecordHistory,
    RecordOutcome, Result, Retention, SettingsChanges, SqliteOptions,
};
use error::SqliteResult;
use rusqlite::Connection;
use std::sync::Arc;
use tokio::sync::Mutex;

pub(super) struct Sqlite {
    // Connection ownership and blocking execution never escape this adapter.
    // Acquire asynchronously before spawning, so blocked requests occupy no thread.
    connection: Arc<Mutex<Option<Connection>>>,
}
impl Sqlite {
    pub async fn open(options: SqliteOptions) -> Result<Self> {
        options.validate()?;
        let connection = tokio::task::spawn_blocking(move || connection::open(options))
            .await
            .map_err(|e| Error::caused_by(ErrorKind::Internal, "open SQLite worker", e))?
            .map_err(|e| e.into_error("open database"))?;
        Ok(Self {
            connection: Arc::new(Mutex::new(Some(connection))),
        })
    }
    async fn run<T: Send + 'static>(
        &self,
        operation: &'static str,
        work: impl FnOnce(&mut Connection) -> SqliteResult<T> + Send + 'static,
    ) -> Result<T> {
        let mut connection = self.connection.clone().lock_owned().await;
        tokio::task::spawn_blocking(move || {
            let connection = connection.as_mut().ok_or_else(Error::stopped)?;
            work(connection).map_err(|e| e.into_error(operation))
        })
        .await
        .map_err(|e| Error::caused_by(ErrorKind::Internal, operation, e))?
    }
}
impl Backend for Sqlite {
    fn record_history(&self, input: RecordHistory) -> BackendFuture<'_, RecordOutcome> {
        Box::pin(self.run("record history", move |db| history::record(db, input)))
    }
    fn query_history(&self, query: HistoryQuery) -> BackendFuture<'_, HistoryPage> {
        Box::pin(self.run("query history", move |db| history::query(db, query)))
    }
    fn get_history(&self, id: HistoryId) -> BackendFuture<'_, Option<HistoryEntry>> {
        Box::pin(self.run("get history", move |db| history::get(db, id)))
    }
    fn delete_history(&self, id: HistoryId) -> BackendFuture<'_, bool> {
        Box::pin(self.run("delete history", move |db| history::delete(db, id)))
    }
    fn prune_history(&self, retention: Retention) -> BackendFuture<'_, u64> {
        Box::pin(self.run("prune history", move |db| history::prune(db, retention)))
    }
    fn clear_history(&self) -> BackendFuture<'_, u64> {
        Box::pin(self.run("clear history", history::clear))
    }
    fn read_settings(&self, keys: Vec<String>) -> BackendFuture<'_, Vec<Option<String>>> {
        Box::pin(self.run("read settings", move |db| settings::read(db, keys)))
    }
    fn apply_settings(&self, changes: SettingsChanges) -> BackendFuture<'_, ()> {
        Box::pin(self.run("apply settings", move |db| settings::apply(db, changes)))
    }
    fn close(&self) -> BackendFuture<'_, ()> {
        Box::pin(async {
            let mut connection = self.connection.clone().lock_owned().await;
            tokio::task::spawn_blocking(move || {
                if let Some(connection) = connection.take() {
                    connection.close().map_err(|(_, error)| {
                        error::Failure::from(error).into_error("close database")
                    })?;
                }
                Ok(())
            })
            .await
            .map_err(|e| Error::caused_by(ErrorKind::Internal, "close SQLite worker", e))?
        })
    }
}
