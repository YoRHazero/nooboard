use super::shutdown::Services;
use crate::{
    configuration::application::runtime as configuration,
    features::clipboard::application::runtime as sync,
    features::clipboard::ports::clipboard::ClipboardPort,
    features::devices::application::runtime as devices,
    features::history::application::runtime as history, *,
};
use nooboard_clipboard::{ClipboardService, Options as ClipboardOptions};
use nooboard_identity::{IdentityConfig, IdentityOptions, IdentityService};
use nooboard_network::NetworkService;
use nooboard_storage::{Options as StorageOptions, StorageService};
use std::sync::Arc;
use tokio::sync::{broadcast, watch};
pub(crate) struct Launch {
    pub options: Options,
    pub defaults: Settings,
    pub identity: IdentityConfig,
    pub clipboard_options: ClipboardOptions,
    pub clipboard: Option<Arc<dyn ClipboardPort>>,
    pub clipboard_service: Option<ClipboardService>,
}
pub(crate) async fn start(options: Options) -> Result<(AppService, App)> {
    let identity = IdentityConfig::System {
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
        let config = crate::configuration::adapters::storage::port(storage.clone())
            .load(input.defaults)
            .await?;
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
        let (identity_service, identity) =
            IdentityService::start(input.identity, IdentityOptions::default()).await?;
        services.identity = Some(identity_service);
        let dependencies =
            crate::app::adapters::network_identity::network_dependencies(identity.clone())?;
        let (network_service, network, network_events) = NetworkService::start(
            nooboard_network::Options {
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
            },
            dependencies,
        )
        .await?;
        services.network = Some(network_service);
        network.set_accepting(config.settings.accepting()).await?;
        network.set_paused(config.settings.paused).await?;
        let terminal_service =
            crate::features::terminal::adapters::create(storage.clone(), identity, network.clone())
                .await?;
        let terminal = terminal_service.workspace.clone();
        let terminal_owner = Arc::new(tokio::sync::Mutex::new(terminal_service));
        let (stop, stopped) = watch::channel(false);
        let (persist_stop, persist_stopped) = watch::channel(false);
        let startup = config.settings.clone();
        let (config, config_runtime) = configuration::channel(config, stopped.clone());
        let (history, history_runtime) = history::channel(stopped.clone());
        let (device, device_runtime) = devices::create(
            crate::features::devices::adapters::network::port(network.clone()),
            Arc::new(config.clone()),
        );
        let (sync, sync_runtime) = sync::create(
            clipboard,
            Arc::new(crate::features::clipboard::adapters::content::ContentAdapter),
            crate::features::clipboard::adapters::network::port(network.clone()),
            Arc::new(config.clone()),
            Arc::new(history.clone()),
            initial,
        );
        static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let session = format!(
            "{}-{}-{}-{}",
            network.status().identity.id,
            crate::clock::now_ms(),
            std::process::id(),
            SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        let initial = crate::app::snapshot::assemble::assemble(
            crate::app::snapshot::assemble::Inputs {
                configuration: &config.current(),
                devices: &device.state.borrow(),
                sync: &sync.state.borrow(),
                history: &history.state.borrow(),
                network: &network.status(),
                startup: &startup,
                terminal: &terminal.snapshot(),
            },
            &session,
            0,
            AppState::Running,
        );
        let (snapshots, receiver) = watch::channel(initial);
        let (events, _) = broadcast::channel(128);
        let app = App {
            terminal: terminal.clone(),
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
                terminal,
                terminal_owner: terminal_owner.clone(),
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
            terminal_owner,
        ))
    }
    .await;
    match result {
        Ok((app, running, stop, terminal_owner)) => {
            let task = tokio::spawn(running.run());
            Ok((
                AppService {
                    terminal_owner,
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
