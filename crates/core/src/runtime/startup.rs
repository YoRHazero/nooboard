use super::shutdown::Services;
use crate::{
    configuration::{document, runtime as configuration},
    devices::runtime as devices,
    history::runtime as history,
    ports::ClipboardPort,
    sync::runtime as sync,
    *,
};
use nooboard_clipboard::{ClipboardService, Options as ClipboardOptions};
use nooboard_network::{IdentityOptions, NetworkService};
use nooboard_storage::{Options as StorageOptions, StorageService};
use std::sync::Arc;
use tokio::sync::{broadcast, watch};
pub(crate) struct Launch {
    pub options: Options,
    pub defaults: Settings,
    pub identity: IdentityOptions,
    pub clipboard_options: ClipboardOptions,
    pub clipboard: Option<Arc<dyn ClipboardPort>>,
    pub clipboard_service: Option<ClipboardService>,
}
pub(crate) async fn start(options: Options) -> Result<(AppService, App)> {
    let identity = IdentityOptions::System {
        profile: options.profile.clone(),
    };
    let defaults = Settings {
        receive_directory: options.default_receive_directory.clone(),
        ..Settings::default()
    };
    launch(Launch {
        options,
        defaults,
        identity,
        clipboard_options: ClipboardOptions::default(),
        clipboard: None,
        clipboard_service: None,
    })
    .await
}
pub(crate) async fn launch(input: Launch) -> Result<(AppService, App)> {
    let mut services = Services {
        clipboard: input.clipboard_service,
        ..Services::default()
    };
    let result = async {
        let (storage_service, storage) =
            StorageService::start(input.options.storage, StorageOptions::default()).await?;
        services.storage = Some(storage_service);
        let config = document::load(&storage, input.defaults).await?;
        let clipboard = match input.clipboard {
            Some(clipboard) => clipboard,
            None => {
                let (service, clipboard) = ClipboardService::start(input.clipboard_options).await?;
                services.clipboard = Some(service);
                Arc::new(clipboard) as Arc<dyn ClipboardPort>
            }
        };
        let initial = clipboard.read().await?;
        // Cached verified addresses allow offline startup. Optional hostname hints
        // are resolved by the device runtime, without blocking unrelated devices.
        let trusted_peers = config
            .peers
            .values()
            .map(|peer| peer.trusted.clone())
            .collect();
        let (network_service, network, network_events) =
            NetworkService::start(nooboard_network::Options {
                identity: input.identity,
                device_name: config.settings.device_name.clone(),
                listen: config
                    .settings
                    .listen_address
                    .parse()
                    .map_err(|_| Error::Configuration)?,
                pairing_listen: config
                    .settings
                    .pairing_listen_address
                    .parse()
                    .map_err(|_| Error::Configuration)?,
                discovery: config.settings.discoverable,
                trusted_peers,
                ..Default::default()
            })
            .await?;
        services.network = Some(network_service);
        network.set_accepting(config.settings.accepting()).await?;
        let (stop, stopped) = watch::channel(false);
        let (persist_stop, persist_stopped) = watch::channel(false);
        let startup = config.settings.clone();
        let (config, config_runtime) = configuration::channel(config, stopped.clone());
        let (history, history_runtime) = history::channel(stopped.clone());
        let (device, device_runtime) = devices::create(network.clone(), config.clone());
        let (sync, sync_runtime) = sync::create(
            clipboard,
            network.clone(),
            config.clone(),
            history.clone(),
            initial,
        );
        static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let session = format!(
            "{}-{}-{}-{}",
            network.status().identity.id,
            crate::history::now_ms(),
            std::process::id(),
            SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        let initial = crate::snapshot::assemble::assemble(
            crate::snapshot::assemble::Inputs {
                configuration: &config.current(),
                devices: &device.state.borrow(),
                sync: &sync.state.borrow(),
                history: &history.state.borrow(),
                network: &network.status(),
                startup: &startup,
            },
            &session,
            0,
            AppState::Running,
        );
        let (snapshots, receiver) = watch::channel(initial);
        let (events, _) = broadcast::channel(128);
        let app = App {
            configuration: config.clone(),
            devices: device.requests.clone(),
            sync: sync.requests.clone(),
            history: history.requests.clone(),
            snapshots: receiver,
            events: events.clone(),
            stopped: stopped.clone(),
            #[cfg(any(test, feature = "diagnostics"))]
            certificate: network.status().identity.certificate.clone(),
        };
        Ok((
            app,
            super::Running {
                services: std::mem::take(&mut services),
                stop: stop.clone(),
                stopped,
                persist_stop,
                persist_stopped,
                config,
                config_runtime,
                history,
                history_runtime,
                device,
                device_runtime,
                sync,
                sync_runtime,
                storage,
                network,
                network_events,
                snapshots,
                events,
                session,
                startup,
            },
            stop,
        ))
    }
    .await;
    match result {
        Ok((app, running, stop)) => {
            let task = tokio::spawn(running.run());
            Ok((
                AppService {
                    stop,
                    task: Some(task),
                },
                app,
            ))
        }
        Err(error) => {
            let _ = services.all().await;
            Err(error)
        }
    }
}
