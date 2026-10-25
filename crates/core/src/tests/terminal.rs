//! Core integration: ephemeral identities, in-memory clipboard and loopback SSH only.
use super::{Clipboard, bounded, start};
use crate::{Error, terminal::*};
use std::sync::atomic::Ordering;
#[allow(dead_code)]
#[path = "../../../terminal/tests/support/mod.rs"]
mod ssh;

async fn state(
    workspace: &TerminalWorkspace,
    predicate: impl Fn(&TerminalWorkspaceSnapshot) -> bool,
) {
    let mut snapshots = workspace.subscribe();
    bounded(async {
        loop {
            if predicate(&snapshots.borrow_and_update()) {
                return;
            }
            snapshots.changed().await.unwrap();
        }
    })
    .await;
}
fn command(text: &str) -> CommandInput {
    CommandInput {
        name: "test command".into(),
        target: CommandTarget::Local,
        command: text.into(),
    }
}
async fn ssh_target(workspace: &TerminalWorkspace, server: &ssh::Server) -> SshTargetConfigId {
    workspace
        .create_ssh_target(SshTargetInput {
            name: "test server".into(),
            address: SshAddressSource::Host(server.address.ip().to_string()),
            port: server.address.port(),
            username: "tester".into(),
            auth: SshAuthInput::Password,
        })
        .await
        .unwrap()
}
async fn ssh_terminal(
    workspace: &TerminalWorkspace,
    target: SshTargetConfigId,
) -> TerminalConfigId {
    workspace
        .create_terminal(TerminalInput {
            name: "remote".into(),
            target: TerminalTarget::Ssh(target),
            dependencies: vec![],
        })
        .await
        .unwrap()
}
async fn prompt(workspace: &TerminalWorkspace) -> TerminalPrompt {
    state(workspace, |s| !s.prompts.is_empty()).await;
    workspace.snapshot().prompts[0].clone()
}
async fn empty(workspace: &TerminalWorkspace) {
    state(workspace, |s| s.runs.is_empty()).await;
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_output_shared_frozen_configuration_and_reliable_occupancy() {
    let (mut service, app) = start(&Clipboard::new()).await;
    let workspace = app.terminal();
    let config = workspace
        .create_command(command("printf old-value; sleep 60"))
        .await
        .unwrap();
    let (run, mut output) = workspace
        .start_command_with_output(config, CommandReusePolicy::Shared, CancellationToken::new())
        .await
        .unwrap();
    bounded(async {
        let mut bytes = output.initial_output.data;
        while !String::from_utf8_lossy(&bytes).contains("old-value") {
            bytes.extend(output.stream.next().await.unwrap().unwrap().data);
        }
    })
    .await;
    workspace
        .update_command(config, command("printf new-value; sleep 60"))
        .await
        .unwrap();
    let reused = workspace
        .start_command(config, CommandReusePolicy::Shared, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(run, reused);
    assert_eq!(
        workspace.snapshot().runs.command_runs[0].command,
        "printf old-value; sleep 60"
    );
    assert!(matches!(
        workspace
            .delete_config(TerminalConfigReference::Command(config))
            .await,
        Err(Error::ConfigurationInUse { .. })
    ));
    workspace.stop_command(run).await.unwrap();
    empty(&workspace).await;
    workspace
        .delete_config(TerminalConfigReference::Command(config))
        .await
        .unwrap();
    bounded(service.shutdown()).await.unwrap();
    assert!(app.snapshot().terminal.runs.is_empty());
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unstarted_dependencies_keep_frozen_data_and_references() {
    let (mut service, app) = start(&Clipboard::new()).await;
    let workspace = app.terminal();
    let gate = workspace.create_command(command("sleep 60")).await.unwrap();
    let later = workspace.create_command(command("sleep 59")).await.unwrap();
    let config = workspace
        .create_terminal(TerminalInput {
            name: "chain".into(),
            target: TerminalTarget::Local,
            dependencies: vec![gate, later]
                .into_iter()
                .map(|command_config_id| DependencyConfig {
                    command_config_id,
                    reuse_policy: CommandReusePolicy::Shared,
                })
                .collect(),
        })
        .await
        .unwrap();
    let root = workspace.start_terminal(config).await.unwrap();
    state(&workspace, |s| {
        s.runs
            .dependency_runs
            .iter()
            .find(|d| d.command_config_id == gate)
            .is_some_and(|d| d.gate_state == DependencyGateState::WaitingForRelease)
    })
    .await;
    workspace
        .update_terminal(
            config,
            TerminalInput {
                name: "edited".into(),
                target: TerminalTarget::Local,
                dependencies: vec![],
            },
        )
        .await
        .unwrap();
    workspace
        .update_command(later, command("exit 0"))
        .await
        .unwrap();
    assert!(matches!(
        workspace
            .delete_config(TerminalConfigReference::Command(later))
            .await,
        Err(Error::ConfigurationInUse { .. })
    ));
    let first = workspace
        .snapshot()
        .runs
        .dependency_runs
        .iter()
        .find(|d| d.command_config_id == gate)
        .unwrap()
        .id;
    workspace.continue_dependency(first).await.unwrap();
    state(&workspace, |s| {
        s.runs.command_runs.iter().any(|c| c.config_id == later)
    })
    .await;
    let snapshot = workspace.snapshot();
    assert_eq!(
        snapshot
            .runs
            .command_runs
            .iter()
            .find(|c| c.config_id == later)
            .unwrap()
            .command,
        "sleep 59"
    );
    // Promoting a dependency keeps its occupancy after its original root exits.
    let child = snapshot
        .runs
        .command_runs
        .iter()
        .find(|c| c.config_id == later)
        .unwrap()
        .id;
    workspace
        .set_command_lifetime_policy(child, CommandLifetimePolicy::Independent)
        .await
        .unwrap();
    workspace.stop_terminal(root).await.unwrap();
    state(&workspace, |s| s.runs.terminal_runs.is_empty()).await;
    assert!(matches!(
        workspace
            .delete_config(TerminalConfigReference::Command(later))
            .await,
        Err(Error::ConfigurationInUse { .. })
    ));
    bounded(service.shutdown()).await.unwrap();
    assert!(workspace.snapshot().runs.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ssh_trust_precedes_password_and_success_remembers_without_exposing_secrets() {
    let server = ssh::Server::start(Default::default()).await;
    let (mut service, app) = start(&Clipboard::new()).await;
    let workspace = app.terminal();
    let target = ssh_target(&workspace, &server).await;
    let config = ssh_terminal(&workspace, target).await;
    let run = workspace.start_terminal(config).await.unwrap();
    let host = prompt(&workspace).await;
    assert!(matches!(host.kind, TerminalPromptKind::HostTrust { .. }));
    assert_eq!(server.data.passwords.load(Ordering::SeqCst), 0);
    workspace.confirm_host(host.id, true).await.unwrap();
    let password = prompt(&workspace).await;
    assert!(matches!(password.kind, TerminalPromptKind::Password { .. }));
    workspace
        .answer_password(password.id, SecretInput::text("test-password".into()), true)
        .await
        .unwrap();
    state(&workspace, |s| {
        s.runs
            .terminal_runs
            .iter()
            .any(|r| r.id == run && r.state == TerminalRunState::Running)
    })
    .await;
    assert!(matches!(
        workspace.snapshot().ssh_targets[0].auth,
        SshAuthSummary::Password { saved: true }
    ));
    let serialized = serde_json::to_string(&workspace.snapshot()).unwrap();
    assert!(!serialized.contains("test-password") && !serialized.contains("ciphertext"));
    // SFTP stays attached to the existing SSH transport.
    workspace
        .list_directory(run, None, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(server.data.transports.load(Ordering::SeqCst), 1);
    assert_eq!(server.data.subsystems.load(Ordering::SeqCst), 1);
    workspace.stop_terminal(run).await.unwrap();
    empty(&workspace).await;
    let next = workspace.start_terminal(config).await.unwrap();
    state(&workspace, |s| {
        s.runs
            .terminal_runs
            .iter()
            .any(|r| r.id == next && r.state == TerminalRunState::Running)
    })
    .await;
    assert!(workspace.snapshot().prompts.is_empty());
    // Explicit replacement is compare-and-set; mismatched host keys never prompt for a password.
    workspace.stop_terminal(next).await.unwrap();
    empty(&workspace).await;
    let wrong = SshPublicKey {
        algorithm: "ssh-ed25519".into(),
        bytes: vec![1, 2, 3],
    };
    workspace
        .replace_host_key(target, Some(server.key.clone()), wrong.clone())
        .await
        .unwrap();
    assert!(
        workspace
            .replace_host_key(target, Some(server.key.clone()), server.key.clone())
            .await
            .is_err()
    );
    workspace.start_terminal(config).await.unwrap();
    state(&workspace, |s| {
        matches!(s.issue, Some(TerminalIssue::HostKeyChanged { .. }))
    })
    .await;
    empty(&workspace).await;
    assert_eq!(server.data.passwords.load(Ordering::SeqCst), 2);
    bounded(service.shutdown()).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rejected_password_and_cancelled_prompts_do_not_persist_credentials() {
    let server = ssh::Server::start(Default::default()).await;
    let (mut service, app) = start(&Clipboard::new()).await;
    let workspace = app.terminal();
    let target = ssh_target(&workspace, &server).await;
    let config = ssh_terminal(&workspace, target).await;
    workspace.start_terminal(config).await.unwrap();
    let host = prompt(&workspace).await;
    workspace.cancel_prompt(host.id).await.unwrap();
    empty(&workspace).await;
    assert!(workspace.confirm_host(host.id, true).await.is_err());
    assert!(workspace.snapshot().ssh_targets[0].trusted_key.is_none());
    workspace.start_terminal(config).await.unwrap();
    workspace
        .confirm_host(prompt(&workspace).await.id, true)
        .await
        .unwrap();
    workspace
        .answer_password(
            prompt(&workspace).await.id,
            SecretInput::text("wrong-password".into()),
            true,
        )
        .await
        .unwrap();
    empty(&workspace).await;
    assert!(matches!(
        workspace.snapshot().ssh_targets[0].auth,
        SshAuthSummary::Password { saved: false }
    ));
    bounded(service.shutdown()).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pending_authentication_cannot_overwrite_a_configuration_edit() {
    let server = ssh::Server::start(Default::default()).await;
    let (mut service, app) = start(&Clipboard::new()).await;
    let workspace = app.terminal();
    let target = ssh_target(&workspace, &server).await;
    let config = ssh_terminal(&workspace, target).await;
    let run = workspace.start_terminal(config).await.unwrap();
    workspace
        .confirm_host(prompt(&workspace).await.id, true)
        .await
        .unwrap();
    let password = prompt(&workspace).await;
    workspace
        .update_ssh_target(
            target,
            SshTargetInput {
                name: "changed".into(),
                address: SshAddressSource::Host(server.address.ip().to_string()),
                port: server.address.port(),
                username: "future-user".into(),
                auth: SshAuthInput::Keep,
            },
        )
        .await
        .unwrap();
    workspace
        .answer_password(password.id, SecretInput::text("test-password".into()), true)
        .await
        .unwrap();
    state(&workspace, |s| {
        s.runs
            .terminal_runs
            .iter()
            .any(|r| r.id == run && r.state == TerminalRunState::Running)
    })
    .await;
    assert!(matches!(
        workspace.snapshot().ssh_targets[0].auth,
        SshAuthSummary::Password { saved: false }
    ));
    assert!(matches!(
        workspace.snapshot().issue,
        Some(TerminalIssue::CredentialNotSaved { .. })
    ));
    bounded(service.shutdown()).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn imported_private_keys_are_unlocked_encrypted_bound_to_id_and_copied_without_trust() {
    use crate::features::terminal::{
        adapters::{
            self,
            identity::SecretsAdapter,
            repository::{KEY, Repository},
        },
        domain::config::SshAuthConfig,
        ports::{
            repository::TerminalRepository,
            secrets::{SecretField, SecretsPort},
        },
    };
    let (storage_owner, storage) = nooboard_storage::StorageService::start(
        crate::BackendConfig::Sqlite(crate::SqliteOptions::in_memory()),
        Default::default(),
    )
    .await
    .unwrap();
    let (mut identity_owner, identity) = nooboard_identity::IdentityService::start(
        nooboard_identity::IdentityConfig::Ephemeral,
        Default::default(),
    )
    .await
    .unwrap();
    let (network_owner, network, _) = super::start_network(nooboard_network::Options {
        listen: "127.0.0.1:0".parse().unwrap(),
        pairing_listen: "127.0.0.1:0".parse().unwrap(),
        discovery: false,
        ..Default::default()
    })
    .await;
    let mut owner = adapters::create(storage.clone(), identity.clone(), network)
        .await
        .unwrap();
    let workspace = &owner.workspace;
    let private =
        russh::keys::PrivateKey::random(&mut rand::rng(), russh::keys::Algorithm::Ed25519).unwrap();
    let encrypted = private
        .encrypt(&mut rand::rng(), "import-passphrase")
        .unwrap()
        .to_openssh(russh::keys::ssh_key::LineEnding::LF)
        .unwrap();
    let make = |password: &str| SshTargetInput {
        name: "key".into(),
        address: SshAddressSource::Host("127.0.0.1".into()),
        port: 22,
        username: "test".into(),
        auth: SshAuthInput::ImportPrivateKey {
            private_key: SecretInput::text(encrypted.to_string()),
            passphrase: Some(SecretInput::text(password.into())),
        },
    };
    assert!(
        workspace
            .create_ssh_target(make("bad-passphrase"))
            .await
            .is_err()
    );
    assert!(workspace.snapshot().ssh_targets.is_empty());
    let id = workspace
        .create_ssh_target(make("import-passphrase"))
        .await
        .unwrap();
    workspace
        .replace_host_key(
            id,
            None,
            SshPublicKey {
                algorithm: "test".into(),
                bytes: vec![1],
            },
        )
        .await
        .unwrap();
    let TerminalConfigReference::SshTarget(copy) = workspace
        .copy_config(TerminalConfigReference::SshTarget(id))
        .await
        .unwrap()
    else {
        panic!()
    };
    let repo = Repository(storage.clone());
    let catalog = repo.load().await.unwrap();
    let key = |id| match &catalog.ssh_targets[&id].auth {
        SshAuthConfig::PrivateKey { private_key } => private_key.clone(),
        _ => panic!(),
    };
    assert_ne!(key(id), key(copy));
    let secrets = SecretsAdapter(identity);
    let opened = secrets
        .open(copy, SecretField::PrivateKey, key(copy))
        .await
        .unwrap();
    let parsed =
        russh::keys::decode_secret_key(std::str::from_utf8(opened.expose()).unwrap(), None)
            .unwrap();
    assert_eq!(parsed.public_key(), private.public_key());
    assert!(
        secrets
            .open(copy, SecretField::PrivateKey, key(id))
            .await
            .is_err()
    );
    assert!(
        secrets
            .open(id, SecretField::Password, key(id))
            .await
            .is_err()
    );
    assert!(!catalog.trusted_hosts.contains_key(&copy));
    let persisted = storage.settings().get(KEY).await.unwrap().unwrap();
    for forbidden in ["import-passphrase", "BEGIN OPENSSH PRIVATE KEY"] {
        assert!(!persisted.contains(forbidden));
    }
    assert!(
        !serde_json::to_string(&workspace.snapshot())
            .unwrap()
            .contains("ciphertext")
    );
    // A failed write cannot publish an unpersisted edit.
    storage_owner.shutdown().await.unwrap();
    assert!(workspace.create_command(command("exit 0")).await.is_err());
    assert!(workspace.snapshot().commands.is_empty());
    bounded(owner.shutdown()).await.unwrap();
    network_owner.shutdown().await.unwrap();
    identity_owner.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn terminal_configuration_and_options_survive_core_restart() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("settings.sqlite");
    let backend = || crate::BackendConfig::Sqlite(crate::SqliteOptions::file(&path));
    let (mut service, app) = super::start_with(&Clipboard::new(), backend(), super::settings())
        .await
        .unwrap();
    let workspace = app.terminal();
    let id = workspace.create_command(command("exit 0")).await.unwrap();
    let mut options = workspace.snapshot().options;
    options.stop.stop_grace_timeout = std::time::Duration::from_millis(321);
    workspace.update_options(options).await.unwrap();
    bounded(service.shutdown()).await.unwrap();
    let (mut service, app) = super::start_with(&Clipboard::new(), backend(), super::settings())
        .await
        .unwrap();
    assert_eq!(app.terminal().snapshot().commands[0].id, id);
    assert_eq!(
        app.terminal().snapshot().options.stop.stop_grace_timeout,
        std::time::Duration::from_millis(321)
    );
    bounded(service.shutdown()).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn device_without_a_current_address_fails_without_pairing_or_authentication() {
    let (mut service, app) = start(&Clipboard::new()).await;
    let workspace = app.terminal();
    let target = workspace
        .create_ssh_target(SshTargetInput {
            name: "unavailable".into(),
            address: SshAddressSource::Device(DeviceId("not-discovered".into())),
            port: 22,
            username: "test".into(),
            auth: SshAuthInput::Password,
        })
        .await
        .unwrap();
    let config = ssh_terminal(&workspace, target).await;
    let mut events = workspace.subscribe_runs().events;
    let run = workspace.start_terminal(config).await.unwrap();
    bounded(async {
        loop {
            if let Some(TerminalEvent::TerminalEnded { run: ended }) = events.next().await.unwrap()
            {
                assert_eq!(ended.id, run);
                assert!(matches!(
                    ended.state,
                    TerminalRunState::Ended {
                        result: TerminalRunResult::Failed(TerminalError {
                            kind: TerminalErrorKind::AddressUnavailable,
                            ..
                        })
                    }
                ));
                break;
            }
        }
    })
    .await;
    assert!(workspace.snapshot().prompts.is_empty());
    bounded(service.shutdown()).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_terminal_shutdown_retains_core_owners_and_can_be_retried() {
    use crate::{contracts::PortFuture, features::terminal::ports::engine::TerminalEngineOwner};
    struct Fail;
    impl TerminalEngineOwner for Fail {
        fn shutdown(&mut self) -> PortFuture<'_, ()> {
            Box::pin(async { Err(Error::Internal) })
        }
    }
    struct Once {
        failed: bool,
        inner: Box<dyn TerminalEngineOwner>,
    }
    impl TerminalEngineOwner for Once {
        fn shutdown(&mut self) -> PortFuture<'_, ()> {
            Box::pin(async move {
                if !self.failed {
                    self.failed = true;
                    Err(Error::Internal)
                } else {
                    self.inner.shutdown().await
                }
            })
        }
    }
    let (mut service, app) = start(&Clipboard::new()).await;
    {
        let mut terminal = service.terminal_owner.lock().await;
        let inner = std::mem::replace(&mut terminal.owner, Box::new(Fail));
        terminal.owner = Box::new(Once {
            failed: false,
            inner,
        });
    }
    let shared_owner = service.terminal_owner.clone();
    let guard = shared_owner.lock().await;
    assert!(matches!(
        bounded(service.shutdown()).await,
        Err(Error::Busy)
    ));
    drop(guard);
    assert!(bounded(service.shutdown()).await.is_err());
    assert!(service.task.is_some());
    assert!(app.terminal().snapshot().closing);
    assert!(matches!(
        app.terminal().snapshot().issue,
        Some(TerminalIssue::Service { .. })
    ));
    // Global stop has not been sent and underlying service use still succeeds.
    assert!(!matches!(
        app.refresh_discovery().await,
        Err(Error::Stopped)
    ));
    app.set_settings(super::settings()).await.unwrap();
    assert!(matches!(
        app.terminal().create_command(command("exit 0")).await,
        Err(Error::Stopped)
    ));
    bounded(service.shutdown()).await.unwrap();
    assert_eq!(app.status().state, crate::AppState::Stopped);
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelling_shared_waiters_releases_pending_configuration_occupancy() {
    let (mut service, app) = start(&Clipboard::new()).await;
    let workspace = app.terminal();
    let mut options = workspace.snapshot().options;
    options.stop.stop_grace_timeout = std::time::Duration::from_secs(1);
    workspace.update_options(options).await.unwrap();
    let id = workspace
        .create_command(command(
            "trap '' HUP TERM; printf '::nooboard:v1:ready::\\n'; while :; do sleep 1; done",
        ))
        .await
        .unwrap();
    let run = workspace
        .start_command(id, CommandReusePolicy::Shared, CancellationToken::new())
        .await
        .unwrap();
    state(&workspace, |s| {
        s.runs
            .command_runs
            .iter()
            .any(|r| r.id == run && r.ready_marker_received)
    })
    .await;
    let stopped = tokio::spawn({
        let workspace = workspace.clone();
        async move { workspace.stop_command(run).await }
    });
    state(&workspace, |s| {
        s.runs
            .command_runs
            .iter()
            .any(|r| r.id == run && matches!(r.state, CommandRunState::Stopping { .. }))
    })
    .await;
    let cancel = CancellationToken::new();
    let waiting = tokio::spawn({
        let workspace = workspace.clone();
        let cancel = cancel.clone();
        async move {
            workspace
                .start_command(id, CommandReusePolicy::Shared, cancel)
                .await
        }
    });
    // Wait for admission using the same serialized coordinator; occupancy must
    // distinguish the pending start from the old stopping run.
    bounded(async {
        loop {
            if matches!(
                workspace
                    .delete_config(TerminalConfigReference::Command(id))
                    .await,
                Err(Error::ConfigurationInUse { pending: true, .. })
            ) {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await;
    cancel.cancel();
    assert!(bounded(waiting).await.unwrap().is_err());
    bounded(stopped).await.unwrap().unwrap();
    empty(&workspace).await;
    workspace
        .delete_config(TerminalConfigReference::Command(id))
        .await
        .unwrap();
    bounded(service.shutdown()).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn successful_authentication_survives_a_failed_password_save() {
    let server = ssh::Server::start(Default::default()).await;
    let (storage_owner, storage) = nooboard_storage::StorageService::start(
        crate::BackendConfig::Sqlite(crate::SqliteOptions::in_memory()),
        Default::default(),
    )
    .await
    .unwrap();
    let (mut identity_owner, identity) = nooboard_identity::IdentityService::start(
        nooboard_identity::IdentityConfig::Ephemeral,
        Default::default(),
    )
    .await
    .unwrap();
    let (network_owner, network, _) = super::start_network(nooboard_network::Options {
        listen: "127.0.0.1:0".parse().unwrap(),
        pairing_listen: "127.0.0.1:0".parse().unwrap(),
        discovery: false,
        ..Default::default()
    })
    .await;
    let mut owner = crate::features::terminal::adapters::create(storage, identity, network)
        .await
        .unwrap();
    let workspace = &owner.workspace;
    let target = ssh_target(workspace, &server).await;
    let config = ssh_terminal(workspace, target).await;
    let run = workspace.start_terminal(config).await.unwrap();
    workspace
        .confirm_host(prompt(workspace).await.id, true)
        .await
        .unwrap();
    let password = prompt(workspace).await;
    storage_owner.shutdown().await.unwrap();
    workspace
        .answer_password(password.id, SecretInput::text("test-password".into()), true)
        .await
        .unwrap();
    state(workspace, |s| {
        s.runs
            .terminal_runs
            .iter()
            .any(|r| r.id == run && r.state == TerminalRunState::Running)
    })
    .await;
    assert!(matches!(
        workspace.snapshot().issue,
        Some(TerminalIssue::CredentialNotSaved { .. })
    ));
    assert!(matches!(
        workspace.snapshot().ssh_targets[0].auth,
        SshAuthSummary::Password { saved: false }
    ));
    bounded(owner.shutdown()).await.unwrap();
    network_owner.shutdown().await.unwrap();
    identity_owner.shutdown().await.unwrap();
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn temporary_local_terminals_do_not_write_or_occupy_saved_configurations() {
    use crate::features::terminal::adapters::{self, repository::KEY};
    let (storage_owner, storage) = nooboard_storage::StorageService::start(
        crate::BackendConfig::Sqlite(crate::SqliteOptions::in_memory()),
        Default::default(),
    )
    .await
    .unwrap();
    let (mut identity_owner, identity) = nooboard_identity::IdentityService::start(
        nooboard_identity::IdentityConfig::Ephemeral,
        Default::default(),
    )
    .await
    .unwrap();
    let (network_owner, network, _) = super::start_network(nooboard_network::Options {
        listen: "127.0.0.1:0".parse().unwrap(),
        pairing_listen: "127.0.0.1:0".parse().unwrap(),
        discovery: false,
        ..Default::default()
    })
    .await;
    let mut owner = adapters::create(storage.clone(), identity, network)
        .await
        .unwrap();
    let workspace = owner.workspace.clone();
    assert!(workspace.start_local_terminal(" ".into()).await.is_err());
    let (first, mut output) = workspace
        .start_local_terminal_with_output("Local shell".into())
        .await
        .unwrap();
    let second = workspace
        .start_local_terminal("Local shell".into())
        .await
        .unwrap();
    assert_ne!(first, second);
    state(&workspace, |s| {
        s.runs.terminal_runs.len() == 2
            && s.runs
                .terminal_runs
                .iter()
                .all(|r| r.state == TerminalRunState::Running)
    })
    .await;
    let snapshot = workspace.snapshot();
    assert!(snapshot.terminals.is_empty());
    assert!(
        snapshot
            .runs
            .terminal_runs
            .iter()
            .all(|r| r.config_id.is_none() && r.name == "Local shell")
    );
    assert!(storage.settings().get(KEY).await.unwrap().is_none());
    workspace
        .write_input(
            TerminalIoTarget::Terminal(first),
            b"printf '%s%s\\n' NOOBOARD_ TEMPORARY_OK\r".to_vec(),
        )
        .await
        .unwrap();
    bounded(async {
        let mut bytes = output.initial_output.data;
        while !String::from_utf8_lossy(&bytes).contains("NOOBOARD_TEMPORARY_OK") {
            bytes.extend(output.stream.next().await.unwrap().unwrap().data);
        }
    })
    .await;

    // A same-named saved configuration remains a separate resource.
    let config = workspace
        .create_terminal(TerminalInput {
            name: "Local shell".into(),
            target: TerminalTarget::Local,
            dependencies: vec![],
        })
        .await
        .unwrap();
    let saved = storage.settings().get(KEY).await.unwrap();
    let third = workspace
        .start_local_terminal("Local shell".into())
        .await
        .unwrap();
    assert_ne!(third, first);
    assert_ne!(third, second);
    assert_eq!(storage.settings().get(KEY).await.unwrap(), saved);
    assert_eq!(workspace.snapshot().terminals.len(), 1);
    assert!(
        workspace
            .snapshot()
            .runs
            .terminal_runs
            .iter()
            .all(|r| r.config_id.is_none())
    );
    let configured = workspace.start_terminal(config).await.unwrap();
    state(&workspace, |s| {
        s.runs.terminal_runs.iter().any(|r| r.id == configured)
    })
    .await;
    assert_eq!(
        workspace
            .snapshot()
            .runs
            .terminal_runs
            .iter()
            .find(|r| r.id == configured)
            .unwrap()
            .config_id,
        Some(config)
    );
    assert!(matches!(
        workspace
            .delete_config(TerminalConfigReference::Terminal(config))
            .await,
        Err(Error::ConfigurationInUse { .. })
    ));
    workspace.stop_terminal(configured).await.unwrap();
    workspace
        .delete_config(TerminalConfigReference::Terminal(config))
        .await
        .unwrap();
    workspace.stop_terminal(first).await.unwrap();
    state(&workspace, |s| s.runs.terminal_runs.len() == 2).await;
    assert!(
        workspace
            .snapshot()
            .runs
            .terminal_runs
            .iter()
            .all(|r| r.id == second || r.id == third)
    );
    bounded(owner.shutdown()).await.unwrap();
    assert!(workspace.snapshot().runs.is_empty());
    assert!(
        workspace
            .start_local_terminal("after shutdown".into())
            .await
            .is_err()
    );
    assert!(workspace.snapshot().terminals.is_empty());
    network_owner.shutdown().await.unwrap();
    identity_owner.shutdown().await.unwrap();
    storage_owner.shutdown().await.unwrap();
}
