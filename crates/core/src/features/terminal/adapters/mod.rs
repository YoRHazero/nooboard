pub(crate) mod engine;
pub(crate) mod identity;
pub(crate) mod keys;
pub(crate) mod network;
pub(crate) mod providers;
pub(crate) mod repository;

pub(crate) async fn create(
    storage: nooboard_storage::Storage,
    identity: nooboard_identity::Identity,
    network: nooboard_network::Network,
) -> crate::Result<super::application::service::TerminalFeatureService> {
    use super::{
        application::{
            api::TerminalWorkspace,
            requests::Coordinator,
            runtime::{Dependencies, Runtime},
            service::TerminalFeatureService,
            state::References,
        },
        ports::{engine::TerminalEngine, repository::TerminalRepository},
    };
    use nooboard_terminal::{CancellationToken, TerminalService, TerminalServiceDependencies};
    use std::sync::Arc;
    let repository = Arc::new(repository::Repository(storage));
    let catalog = repository.load().await?;
    let (requests, inbox) = tokio::sync::mpsc::channel(catalog.options.control_queue_capacity);
    let coordinator = Coordinator(requests);
    let (state, view) = tokio::sync::watch::channel(catalog.view());
    let secrets = Arc::new(identity::SecretsAdapter(identity));
    let references = References::default();
    let providers = Arc::new(providers::Providers {
        coordinator: coordinator.clone(),
        secrets: secrets.clone(),
        devices: Arc::new(network::DeviceAddressAdapter(network)),
    });
    let owner = TerminalService::start_with_leases(
        catalog.options.clone(),
        TerminalServiceDependencies {
            host_verifier: Arc::new(providers::HostVerifier(coordinator.clone())),
        },
        Arc::new(providers::Leases(references.clone())),
    )?;
    let engine: Arc<dyn TerminalEngine> = Arc::new(engine::EngineAdapter {
        handle: owner.handle(),
        providers,
    });
    let stop = CancellationToken::new();
    let runtime = Runtime::new(
        catalog,
        inbox,
        state,
        Dependencies {
            engine: engine.clone(),
            repository,
            secrets,
            keys: Arc::new(keys::PrivateKeyAdapter),
            references,
        },
    );
    let token = stop.clone();
    let task = tokio::spawn(runtime.run(token));
    Ok(TerminalFeatureService {
        workspace: TerminalWorkspace {
            coordinator,
            engine,
            state: view,
        },
        owner: Box::new(engine::EngineOwner(owner)),
        task: Some(task),
        stop,
    })
}
