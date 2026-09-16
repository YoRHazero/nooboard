use crate::{
    App, Error, Options, Result, Status,
    devices::Configuration,
    ports::{ClipboardPort, Store},
    runtime::Runtime,
};
use nooboard_clipboard::Clipboard;
use nooboard_network::{Identity, MAX_TEXT_BYTES};
use nooboard_storage::{Database, secrets::SecretStore};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, watch};

pub(crate) async fn start(options: Options) -> Result<App> {
    if options.profile.is_empty() {
        return Err(Error::Configuration);
    }
    let (database, identity, clipboard) = tokio::task::spawn_blocking(move || -> Result<_> {
        let database = Database::open(&options.database)?;
        let secrets = SecretStore::new(&options.profile)?;
        let identity = match secrets.get()? {
            Some(secret) => Identity::from_secret(&secret)?,
            None => {
                let identity = Identity::generate()?;
                secrets.set(&identity.export_secret())?;
                identity
            }
        };
        let clipboard = Clipboard::open(Duration::from_millis(250), MAX_TEXT_BYTES)?;
        Ok((database, identity, clipboard))
    })
    .await
    .map_err(|_| Error::Stopped)??;
    let app = start_parts(database, identity, Box::new(clipboard)).await?;
    app.refresh_discovery().await?;
    Ok(app)
}
pub(crate) async fn start_parts(
    database: Database,
    identity: Identity,
    clipboard: Box<dyn ClipboardPort>,
) -> Result<App> {
    let store = Store::new(database);
    let configuration = Configuration::load(&store, &identity).await?;
    crate::history::prune(&store, &configuration.settings).await?;
    let initial = Status {
        noob_id: identity.noob_id()?,
        fingerprint: identity.fingerprint(),
        listen_address: configuration.settings.listen_address.clone(),
        settings: configuration.settings.clone(),
        peers: Vec::new(),
        manual_targets: configuration.manual_targets.clone(),
        transfers: Vec::new(),
    };
    #[cfg(any(test, feature = "diagnostics"))]
    let certificate = identity.certificate().to_vec();
    let (commands, requests) = mpsc::channel(16);
    let (status_sender, status) = watch::channel(initial.clone());
    let (events, _) = broadcast::channel(128);
    let runtime = Runtime::new(
        store,
        identity,
        clipboard,
        configuration,
        status_sender,
        events.clone(),
    )
    .await?;
    let snapshots = runtime.snapshots.subscribe();
    let task = tokio::spawn(runtime.run(requests));
    Ok(App {
        commands,
        status,
        snapshots,
        events,
        #[cfg(any(test, feature = "diagnostics"))]
        certificate,
        task: Some(task),
    })
}
