use super::*;
use crate::configuration::runtime::Change;
use crate::remote::model::{ConfigurationChange, host_id};
use std::sync::atomic::Ordering;
// Share the real SSH/SFTP test server; production core never imports a driver.
#[allow(dead_code)]
#[path = "../../../ssh/tests/support/mod.rs"]
mod server;

async fn setup(data: server::Data) -> (AppService, AppService, App, App, server::Server) {
    let (owner, app) = start(&Clipboard::new()).await;
    let (peer_owner, peer) = start(&Clipboard::new()).await;
    app.trust_peer(fixture(&peer)).await.unwrap();
    let server = server::Server::start(data).await;
    (owner, peer_owner, app, peer, server)
}
fn login(peer: &App, server: &server::Server, remember: bool) -> RemoteLogin {
    RemoteLogin {
        workspace: None,
        device_id: peer.status().noob_id,
        username: "tester".into(),
        port: server.address.port(),
        password: Some(SshPassword::new("test-password".into())),
        remember,
    }
}
async fn wait_remote(remote: &Remote, predicate: impl Fn(&RemoteSnapshot) -> bool) {
    let mut state = remote.subscribe();
    bounded(async {
        loop {
            if predicate(&state.borrow_and_update()) {
                return;
            }
            state.changed().await.unwrap();
        }
    })
    .await;
}
async fn accept(remote: &Remote) {
    wait_remote(remote, |s| !s.challenges.is_empty()).await;
    let challenge = remote.snapshot().challenges[0].id.clone();
    remote.decide_host_key(challenge, true).await.unwrap();
}
async fn ready(remote: &Remote) {
    wait_remote(remote, |s| {
        s.workspaces.iter().any(|w| {
            w.terminal == SshConnectionState::Ready && w.files == SshConnectionState::Ready
        })
    })
    .await;
}
#[tokio::test]
async fn host_trust_is_saved_before_authentication_and_directory_expansion_is_lazy() {
    let (owner, peer_owner, app, peer, server) = setup(Default::default()).await;
    let remote = app.remote();
    let id = remote.connect(login(&peer, &server, false)).await.unwrap();
    wait_remote(&remote, |s| !s.challenges.is_empty()).await;
    assert_eq!(server.data.passwords.load(Ordering::SeqCst), 0);
    assert!(app.configuration.current().remote.hosts.is_empty());
    accept(&remote).await;
    ready(&remote).await;
    assert_eq!(
        app.configuration.current().remote.hosts
            [&host_id(&peer.status().noob_id, server.address.port())],
        server.key
    );
    assert_eq!(server.data.passwords.load(Ordering::SeqCst), 2);
    assert_eq!(server.data.reads.load(Ordering::SeqCst), 0);
    let root = remote.snapshot().workspaces[0].root.clone().unwrap();
    let old_target = target(&remote, &id);
    remote
        .navigate(target(&remote, &id), root.as_str().into())
        .await
        .unwrap();
    assert_eq!(server.data.reads.load(Ordering::SeqCst), 0);
    assert!(
        remote
            .list_directory(old_target, root.clone(), false)
            .await
            .is_err()
    );
    assert_eq!(server.data.reads.load(Ordering::SeqCst), 0);
    remote
        .list_directory(target(&remote, &id), root.clone(), false)
        .await
        .unwrap();
    assert_eq!(
        remote.snapshot().workspaces[0].directories[0].entries.len(),
        200
    );
    assert_eq!(server.data.reads.load(Ordering::SeqCst), 2);
    assert!(
        remote
            .navigate(target(&remote, &id), "/missing".into())
            .await
            .is_err()
    );
    assert_eq!(remote.snapshot().workspaces[0].root, Some(root.clone()));
    remote
        .list_directory(target(&remote, &id), root.clone(), true)
        .await
        .unwrap();
    assert_eq!(
        remote.snapshot().workspaces[0].directories[0].entries.len(),
        400
    );
    remote
        .collapse_directory(target(&remote, &id), root)
        .await
        .unwrap();
    owner.shutdown().await.unwrap();
    peer_owner.shutdown().await.unwrap();
}
#[tokio::test]
async fn remembered_credentials_require_success_and_a_changed_key_never_receives_passwords() {
    let (owner, peer_owner, app, peer, server) = setup(Default::default()).await;
    let remote = app.remote();
    let id = remote.connect(login(&peer, &server, true)).await.unwrap();
    assert!(!remote.snapshot().profiles[0].remembered());
    accept(&remote).await;
    ready(&remote).await;
    wait_remote(&remote, |s| s.profiles[0].remembered()).await;
    remote.close_workspace(id).await.unwrap();
    let mut changed = server.key.clone();
    changed.public_key.push_str("changed");
    app.configuration
        .change(Change::Remote(ConfigurationChange::Host(
            host_id(&peer.status().noob_id, server.address.port()),
            Some(changed),
        )))
        .await
        .unwrap();
    let before = server.data.passwords.load(Ordering::SeqCst);
    let mut request = login(&peer, &server, true);
    request.password = None;
    remote.connect(request).await.unwrap();
    wait_remote(&remote, |s| {
        s.workspaces
            .iter()
            .any(|w| w.terminal == SshConnectionState::Failed(SshError::HostKeyChanged))
    })
    .await;
    assert_eq!(server.data.passwords.load(Ordering::SeqCst), before);
    assert!(remote.snapshot().profiles.iter().all(|p| !p.remembered()));
    assert!(
        app.configuration
            .current()
            .remote
            .profiles
            .values()
            .all(|p| !p.remembered())
    );
    let document = serde_json::to_string(&app.configuration.current()).unwrap();
    assert!(!document.contains("test-password"));
    owner.shutdown().await.unwrap();
    peer_owner.shutdown().await.unwrap();
}
#[tokio::test]
async fn queued_uploads_survive_tab_close_and_download_targets_are_frozen() {
    let (owner, peer_owner, app, peer, server) = setup(server::Data {
        slow_writes: true,
        ..Default::default()
    })
    .await;
    let remote = app.remote();
    let id = remote.connect(login(&peer, &server, false)).await.unwrap();
    accept(&remote).await;
    ready(&remote).await;
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("sample.txt");
    std::fs::write(&source, vec![7; 100_000]).unwrap();
    let root = remote.snapshot().workspaces[0].root.clone().unwrap();
    remote
        .upload(
            target(&remote, &id),
            root,
            vec![source.clone(), source.clone(), source],
        )
        .await
        .unwrap();
    wait_remote(&remote, |s| s.transfers.len() == 3).await;
    remote.close_workspace(id).await.unwrap();
    wait_remote(&remote, |s| {
        s.transfers.len() == 3
            && s.transfers
                .iter()
                .all(|t| t.state == RemoteTransferState::Succeeded)
    })
    .await;
    assert!(remote.snapshot().workspaces.is_empty());
    let id = remote.connect(login(&peer, &server, false)).await.unwrap();
    ready(&remote).await;
    let first = temp.path().join("first");
    let second = temp.path().join("second");
    std::fs::create_dir_all(&first).unwrap();
    std::fs::create_dir_all(&second).unwrap();
    remote.set_download_directory(first.clone()).await.unwrap();
    let remote_path = remote.snapshot().transfers[0].target.clone().unwrap();
    remote
        .download(
            target(&remote, &id),
            vec![RemotePath::new(remote_path).unwrap()],
        )
        .await
        .unwrap();
    remote.set_download_directory(second.clone()).await.unwrap();
    wait_remote(&remote, |s| {
        s.transfers.len() == 4
            && s.transfers
                .iter()
                .all(|t| t.state == RemoteTransferState::Succeeded)
    })
    .await;
    let saved = remote
        .snapshot()
        .transfers
        .into_iter()
        .find(|t| matches!(t.spec, RemoteTransferSpec::Download { .. }))
        .unwrap()
        .target
        .unwrap();
    assert!(std::path::Path::new(&saved).starts_with(&first));
    assert_eq!(std::fs::read(saved).unwrap(), vec![7; 100_000]);
    assert!(std::fs::read_dir(second).unwrap().next().is_none());
    owner.shutdown().await.unwrap();
    peer_owner.shutdown().await.unwrap();
}
#[tokio::test]
async fn v3_migrates_without_changing_existing_settings_and_new_versions_are_preserved() {
    use nooboard_storage::{Setting, SettingsChanges, StorageService};
    let (owner, storage) = StorageService::start(
        BackendConfig::Sqlite(SqliteOptions::in_memory()),
        Default::default(),
    )
    .await
    .unwrap();
    let old = serde_json::json!({"version":3,"settings":settings(),"peers":{},"manual_targets":[]});
    let key = crate::configuration::document::KEY;
    for version in [3, 99] {
        let mut doc = old.clone();
        doc["version"] = version.into();
        let text = doc.to_string();
        storage
            .settings()
            .apply(SettingsChanges {
                put: vec![Setting {
                    key: key.into(),
                    value: text.clone(),
                }],
                delete: vec![],
            })
            .await
            .unwrap();
        let loaded = crate::configuration::document::load(&storage, settings()).await;
        if version == 3 {
            let loaded = loaded.unwrap();
            assert_eq!(loaded.version, 4);
            assert_eq!(loaded.settings, settings());
            assert!(loaded.remote.profiles.is_empty());
        } else {
            assert!(loaded.is_err());
            assert_eq!(storage.settings().get(key).await.unwrap(), Some(text));
        }
    }
    owner.shutdown().await.unwrap();
}

#[tokio::test]
async fn separate_reconnections_preserve_the_other_transport_and_each_failure_revokes_saved_login()
{
    let (owner, peer_owner, app, peer, server) = setup(Default::default()).await;
    let remote = app.remote();
    let id = remote.connect(login(&peer, &server, true)).await.unwrap();
    accept(&remote).await;
    ready(&remote).await;
    wait_remote(&remote, |s| s.profiles[0].remembered()).await;
    let original = remote.snapshot().workspaces[0].clone();
    let files = server.data.file_connections.lock().unwrap()[0].clone();
    files
        .disconnect(
            russh::Disconnect::ByApplication,
            "files test".into(),
            "en".into(),
        )
        .await
        .unwrap();
    wait_remote(&remote, |s| {
        matches!(s.workspaces[0].files, SshConnectionState::Failed(_))
            && !s.profiles[0].remembered()
    })
    .await;
    let mut request = login(&peer, &server, true);
    request.workspace = Some(id.clone());
    remote.connect(request).await.unwrap();
    ready(&remote).await;
    wait_remote(&remote, |s| s.profiles[0].remembered()).await;
    let replaced = remote.snapshot().workspaces[0].clone();
    assert_eq!(replaced.terminal_id, original.terminal_id);
    assert_ne!(replaced.files_id, original.files_id);
    let terminal = server.data.terminals.lock().unwrap()[0].clone();
    terminal
        .disconnect(
            russh::Disconnect::ByApplication,
            "terminal test".into(),
            "en".into(),
        )
        .await
        .unwrap();
    wait_remote(&remote, |s| {
        matches!(s.workspaces[0].terminal, SshConnectionState::Failed(_))
            && !s.profiles[0].remembered()
    })
    .await;
    let mut request = login(&peer, &server, false);
    request.workspace = Some(id.clone());
    remote.connect(request).await.unwrap();
    ready(&remote).await;
    let current = remote.snapshot().workspaces[0].clone();
    assert_eq!(current.files_id, replaced.files_id);
    assert_ne!(current.terminal_id, original.terminal_id);
    assert!(
        remote
            .write_terminal(id.clone(), original.terminal_id.clone(), b"stale".to_vec())
            .await
            .is_err()
    );
    assert!(
        remote
            .terminal_replay(id, original.terminal_id, 0)
            .await
            .is_err()
    );
    owner.shutdown().await.unwrap();
    peer_owner.shutdown().await.unwrap();
}

fn target(remote: &Remote, workspace: &str) -> RemoteFileTarget {
    let w = remote
        .snapshot()
        .workspaces
        .into_iter()
        .find(|w| w.id == workspace)
        .unwrap();
    RemoteFileTarget {
        workspace: w.id,
        files: w.files_id,
        generation: w.generation,
    }
}

#[test]
fn discovered_pairing_addresses_use_the_latest_ip_and_the_ssh_port() {
    use crate::remote::connection::discovered_address;
    assert_eq!(
        discovered_address("192.168.1.42:24817", 22)
            .unwrap()
            .to_string(),
        "192.168.1.42:22"
    );
    assert_eq!(
        discovered_address("[2001:db8::2]:24817", 2222)
            .unwrap()
            .to_string(),
        "[2001:db8::2]:2222"
    );
    assert_eq!(
        discovered_address("192.168.1.43", 22).unwrap().to_string(),
        "192.168.1.43:22"
    );
    assert!(discovered_address("not-an-address", 22).is_none());
}
