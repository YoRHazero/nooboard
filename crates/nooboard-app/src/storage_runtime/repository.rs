use nooboard_storage::SqliteEventRepository;

use crate::AppResult;

pub(super) fn open_repository(
    storage_config: &nooboard_storage::AppConfig,
) -> AppResult<SqliteEventRepository> {
    let mut repository = SqliteEventRepository::open(storage_config.clone())?;
    repository.init_storage()?;
    Ok(repository)
}
