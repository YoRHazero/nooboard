use crate::{
    Error,
    contracts::PortFuture,
    features::terminal::{domain::config::Catalog, ports::repository::TerminalRepository},
};
pub(crate) const KEY: &str = "core.terminal.configuration";
pub(crate) struct Repository(pub nooboard_storage::Storage);
impl TerminalRepository for Repository {
    fn load(&self) -> PortFuture<'_, Catalog> {
        Box::pin(async move {
            let catalog = match self.0.settings().get(KEY).await? {
                None => Catalog::default(),
                Some(text) => serde_json::from_str(&text).map_err(|_| Error::Configuration)?,
            };
            catalog.validate()?;
            Ok(catalog)
        })
    }
    fn save(&self, catalog: Catalog) -> PortFuture<'_, ()> {
        Box::pin(async move {
            catalog.validate()?;
            self.0
                .settings()
                .apply(nooboard_storage::SettingsChanges {
                    put: vec![nooboard_storage::Setting {
                        key: KEY.into(),
                        value: serde_json::to_string(&catalog).map_err(|_| Error::Configuration)?,
                    }],
                    delete: vec![],
                })
                .await?;
            Ok(())
        })
    }
}
