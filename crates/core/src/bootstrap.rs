use crate::{
    App, Error, Options, Result, Settings, Status,
    model::Peer,
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
    start_parts(database, identity, Box::new(clipboard)).await
}
pub(crate) async fn start_parts(
    database: Database,
    identity: Identity,
    clipboard: Box<dyn ClipboardPort>,
) -> Result<App> {
    let store = Store::new(database);
    let (settings, peer) = store
        .run(|db| Ok((db.setting("settings")?, db.setting("peer")?)))
        .await?;
    let settings: Settings = settings
        .map(|bytes| serde_json::from_slice(&bytes))
        .transpose()
        .map_err(|_| Error::Configuration)?
        .unwrap_or_default();
    settings.validate()?;
    let peer: Option<Peer> = peer
        .map(|bytes| serde_json::from_slice(&bytes))
        .transpose()
        .map_err(|_| Error::Configuration)?;
    if let Some(peer) = &peer {
        nooboard_network::TlsConfig::new(&identity, &peer.certificate)?;
    }
    crate::history::prune(&store, &settings).await?;
    let initial = Status {
        fingerprint: identity.fingerprint(),
        peer_fingerprint: peer
            .as_ref()
            .map(|p| nooboard_network::fingerprint(&p.certificate)),
        online: false,
        peer_accepting: false,
        settings,
    };
    let certificate = identity.certificate().to_vec();
    let (commands, requests) = mpsc::channel(16);
    let (status_sender, status) = watch::channel(initial.clone());
    let (events, _) = broadcast::channel(128);
    let runtime = Runtime::new(
        store,
        identity,
        clipboard,
        peer,
        initial,
        status_sender,
        events.clone(),
    );
    let task = tokio::spawn(runtime.run(requests));
    Ok(App {
        commands,
        status,
        events,
        certificate,
        task: Some(task),
    })
}
