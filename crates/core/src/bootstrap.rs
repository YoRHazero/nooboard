use crate::{
    App, Error, Options, Result, Status,
    devices::Configuration,
    ports::{ClipboardPort, Store},
    runtime::Runtime,
};
use nooboard_clipboard::{ClipboardService, Limits, Options as ClipboardOptions};
use nooboard_network::{Identity, MAX_TEXT_BYTES};
use nooboard_storage::{Database, secrets::SecretStore};
use std::path::PathBuf;
use tokio::sync::{broadcast, mpsc, watch};

pub(crate) async fn start(options: Options) -> Result<App> {
    if options.profile.is_empty() {
        return Err(Error::Configuration);
    }
    let (database, identity) = tokio::task::spawn_blocking(move || -> Result<_> {
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
        Ok((database, identity))
    })
    .await
    .map_err(|_| Error::Stopped)??;
    let (service, clipboard) = ClipboardService::start(ClipboardOptions {
        limits: Limits {
            text_bytes: MAX_TEXT_BYTES,
            ..Limits::default()
        },
        ..ClipboardOptions::default()
    })
    .await?;
    let mut app = start_parts(
        database,
        identity,
        Box::new(clipboard),
        options.default_receive_directory,
    )
    .await?;
    app.clipboard_service = Some(service);
    app.refresh_discovery().await?;
    Ok(app)
}
pub(crate) async fn start_parts(
    database: Database,
    identity: Identity,
    clipboard: Box<dyn ClipboardPort>,
    default_receive_directory: Option<PathBuf>,
) -> Result<App> {
    let store = Store::new(database);
    let configuration = Configuration::load(&store, &identity, default_receive_directory).await?;
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
        clipboard_service: None,
        commands,
        status,
        snapshots,
        events,
        #[cfg(any(test, feature = "diagnostics"))]
        certificate,
        task: Some(task),
    })
}
