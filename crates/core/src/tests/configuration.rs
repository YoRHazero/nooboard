use super::*;
#[tokio::test]
async fn rejected_persistence_does_not_publish_unsaved_settings_and_restart_requirement_is_honest()
{
    let clipboard = Clipboard::new();
    let (service, app) = start(&clipboard).await;
    let original = app.status().settings;
    let directory = tempfile::tempdir().unwrap();
    let mut oversized = original.clone();
    // A rooted `/...` path is not absolute on Windows. Pass configuration
    // validation on every platform, then exercise the storage value limit.
    oversized.receive_directory = Some(directory.path().join("a".repeat(1024 * 1024)));
    oversized.validate().unwrap();
    let error = app.set_settings(oversized).await.unwrap_err();
    assert!(
        matches!(
            &error,
            Error::Storage(error) if error.kind() == nooboard_storage::ErrorKind::InvalidInput
        ),
        "expected storage to reject the oversized setting: {error:?}"
    );
    assert_eq!(app.status().settings, original);
    app.set_settings(Settings {
        device_name: "new name".into(),
        ..original
    })
    .await
    .unwrap();
    assert!(app.status().restart_required);
    assert_eq!(app.status().settings.device_name, "new name");
    service.shutdown().await.unwrap();
}
#[tokio::test]
async fn configuration_and_history_persist_through_service_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.sqlite");
    let clipboard = Clipboard::new();
    let (service, app) = start_with(
        &clipboard,
        BackendConfig::Sqlite(SqliteOptions::file(&path)),
        settings(),
    )
    .await
    .unwrap();
    app.set_settings(Settings {
        history_days: 12,
        device_name: "persisted".into(),
        ..app.status().settings
    })
    .await
    .unwrap();
    clipboard.copy(Payload::Text("stored history".into()));
    wait(&app, |s| s.history_revision > 0).await;
    service.shutdown().await.unwrap();
    let (service, app) = start_with(
        &clipboard,
        BackendConfig::Sqlite(SqliteOptions::file(&path)),
        settings(),
    )
    .await
    .unwrap();
    assert_eq!(app.status().settings.history_days, 12);
    assert!(!app.status().restart_required);
    assert_eq!(
        app.history(String::new(), 20, 0).await.unwrap()[0].text,
        "stored history"
    );
    service.shutdown().await.unwrap();
}
#[tokio::test]
async fn concurrent_device_configuration_changes_merge_against_latest_document() {
    let a = Clipboard::new();
    let b = Clipboard::new();
    let c = Clipboard::new();
    let (sa, aa) = start(&a).await;
    let (sb, ab) = start(&b).await;
    let (sc, ac) = start(&c).await;
    let (x, y) = tokio::join!(aa.trust_peer(fixture(&ab)), aa.trust_peer(fixture(&ac)));
    x.unwrap();
    y.unwrap();
    wait(&aa, |s| s.status.peers.len() == 2).await;
    let (x, y) = tokio::join!(
        aa.select_targets(vec![ab.status().noob_id]),
        aa.set_settings(Settings {
            history_days: 9,
            ..aa.status().settings
        })
    );
    x.unwrap();
    y.unwrap();
    assert_eq!(aa.status().peers.len(), 2);
    assert_eq!(aa.status().manual_targets.len(), 1);
    assert_eq!(aa.status().settings.history_days, 9);
    sa.shutdown().await.unwrap();
    sb.shutdown().await.unwrap();
    sc.shutdown().await.unwrap();
}

#[tokio::test]
async fn legacy_registry_migrates_public_trust_atomically_and_corrupt_documents_are_preserved() {
    use nooboard_storage::{Setting, SettingsChanges, StorageService};
    let clipboard = Clipboard::new();
    let (service, app) = start(&clipboard).await;
    let peer = fixture(&app);
    let (owner, storage) = StorageService::start(
        BackendConfig::Sqlite(SqliteOptions::in_memory()),
        Default::default(),
    )
    .await
    .unwrap();
    let old = serde_json::json!({
        "version": 2, "settings": settings(),
        "peers": { peer.noob_id.clone(): {
            "noob_id": peer.noob_id, "device_name": peer.device_name,
            "certificate": peer.certificate, "settings": {"address": peer.address, "auto_send": true}
        }}, "manual_targets": [peer.noob_id]
    }).to_string();
    storage
        .settings()
        .apply(SettingsChanges {
            put: vec![Setting {
                key: "configuration_v2".into(),
                value: old.clone(),
            }],
            delete: vec![],
        })
        .await
        .unwrap();
    let config = crate::configuration::document::load(&storage, settings())
        .await
        .unwrap();
    assert_eq!(config.version, 3);
    assert_eq!(config.manual_targets, vec![peer.noob_id.clone()]);
    assert_eq!(
        config.peers[&peer.noob_id].trusted.identity.fingerprint,
        peer.confirmed_fingerprint
    );
    assert!(config.peers[&peer.noob_id].settings.auto_send);
    assert!(
        storage
            .settings()
            .get("configuration_v2")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        storage
            .settings()
            .get(crate::configuration::document::KEY)
            .await
            .unwrap()
            .is_some()
    );
    let mut corrupt: serde_json::Value = serde_json::from_str(&old).unwrap();
    corrupt["peers"][&peer.noob_id]["noob_id"] = serde_json::Value::String("wrong identity".into());
    let corrupt = corrupt.to_string();
    storage
        .settings()
        .apply(SettingsChanges {
            put: vec![Setting {
                key: "configuration_v2".into(),
                value: corrupt.clone(),
            }],
            delete: vec![crate::configuration::document::KEY.into()],
        })
        .await
        .unwrap();
    assert!(
        crate::configuration::document::load(&storage, settings())
            .await
            .is_err()
    );
    assert_eq!(
        storage.settings().get("configuration_v2").await.unwrap(),
        Some(corrupt)
    );
    assert!(
        storage
            .settings()
            .get(crate::configuration::document::KEY)
            .await
            .unwrap()
            .is_none()
    );
    owner.shutdown().await.unwrap();
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn authenticated_peer_name_is_saved_without_changing_trust_or_device_preferences() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("app.sqlite");
    let a = Clipboard::new();
    let b = Clipboard::new();
    let (sa, aa) = start_with(
        &a,
        BackendConfig::Sqlite(SqliteOptions::file(&path)),
        settings(),
    )
    .await
    .unwrap();
    let (sb, ab) = start_with(
        &b,
        BackendConfig::Sqlite(SqliteOptions::in_memory()),
        Settings {
            device_name: "Renamed device 新名称".into(),
            ..settings()
        },
    )
    .await
    .unwrap();
    // The stored pairing name is stale; the actual peer advertises its new name
    // only after establishing the authenticated connection.
    let mut old = fixture(&ab);
    old.device_name = "Pairing-time name".into();
    let id = old.noob_id.clone();
    let fingerprint = old.confirmed_fingerprint.clone();
    aa.trust_peer(old.clone()).await.unwrap();
    let preferences = PeerSettings {
        address: old.address,
        auto_send: true,
    };
    aa.configure_peer(id.clone(), preferences.clone())
        .await
        .unwrap();
    aa.select_targets(vec![id.clone()]).await.unwrap();
    ab.trust_peer(fixture(&aa)).await.unwrap();
    let expected = ab.status().settings.device_name;
    wait(&aa, |s| {
        s.status.peers.iter().any(|p| p.device_name == expected)
    })
    .await;
    let saved = aa.configuration.current();
    assert_eq!(saved.peers[&id].trusted.identity.device_name, expected);
    assert_eq!(saved.peers[&id].trusted.identity.fingerprint, fingerprint);
    assert_eq!(
        saved.peers[&id].trusted.identity.certificate,
        old.certificate
    );
    assert_eq!(saved.peers[&id].settings, preferences);
    assert_eq!(saved.manual_targets, vec![id.clone()]);

    sb.shutdown().await.unwrap();
    wait(&aa, |s| s.status.peers.iter().all(|p| !p.online)).await;
    assert_eq!(aa.status().peers[0].device_name, expected);
    sa.shutdown().await.unwrap();
    let (sa, aa) = start_with(
        &a,
        BackendConfig::Sqlite(SqliteOptions::file(&path)),
        settings(),
    )
    .await
    .unwrap();
    assert_eq!(aa.status().peers[0].device_name, expected);
    // A delayed metadata update never restores a removed trust record.
    aa.unpair(id.clone()).await.unwrap();
    aa.configuration
        .change(crate::configuration::runtime::Change::PeerNames(vec![(
            id,
            "late name".into(),
        )]))
        .await
        .unwrap();
    assert!(aa.configuration.current().peers.is_empty());
    sa.shutdown().await.unwrap();
}

#[tokio::test]
async fn every_document_format_fills_only_a_missing_receive_directory_and_persists_it() {
    use crate::configuration::{document, model::Configuration};
    use nooboard_storage::{Setting, SettingsChanges, StorageService};
    let directory = tempfile::tempdir().unwrap();
    let fallback = directory.path().join("host-default");
    let explicit = directory.path().join("user-selected");
    for key in [document::KEY, "configuration_v2", "settings"] {
        for user_selected in [false, true] {
            let (owner, storage) = StorageService::start(
                BackendConfig::Sqlite(SqliteOptions::in_memory()),
                Default::default(),
            )
            .await
            .unwrap();
            let original = Settings {
                receive_directory: user_selected.then(|| explicit.clone()),
                history_days: 12,
                ..settings()
            };
            let value = match key {
                "settings" => serde_json::to_string(&original).unwrap(),
                "configuration_v2" => serde_json::json!({
                    "version": 2, "settings": original,
                    "peers": {}, "manual_targets": []
                })
                .to_string(),
                _ => serde_json::to_string(&Configuration::new(original.clone())).unwrap(),
            };
            storage
                .settings()
                .apply(SettingsChanges {
                    put: vec![Setting {
                        key: key.into(),
                        value,
                    }],
                    delete: vec![],
                })
                .await
                .unwrap();
            let loaded = document::load(
                &storage,
                Settings {
                    receive_directory: Some(fallback.clone()),
                    ..settings()
                },
            )
            .await
            .unwrap();
            let expected = Settings {
                receive_directory: Some(if user_selected {
                    explicit.clone()
                } else {
                    fallback.clone()
                }),
                ..original
            };
            assert_eq!(loaded.settings, expected, "{key}");
            let saved: Configuration = serde_json::from_str(
                &storage
                    .settings()
                    .get(document::KEY)
                    .await
                    .unwrap()
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(saved.settings, expected);
            let reopened = document::load(&storage, settings()).await.unwrap();
            assert_eq!(reopened.settings, expected);
            owner.shutdown().await.unwrap();
        }
    }
}

#[tokio::test]
async fn existing_configuration_can_receive_files_in_the_host_default_directory() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("app.sqlite");
    let receive = directory.path().join("received");
    let b = Clipboard::new();
    let (sb, _) = start_with(
        &b,
        BackendConfig::Sqlite(SqliteOptions::file(&path)),
        settings(),
    )
    .await
    .unwrap();
    sb.shutdown().await.unwrap();
    let (sb, ab) = start_with(
        &b,
        BackendConfig::Sqlite(SqliteOptions::file(&path)),
        Settings {
            receive_directory: Some(receive.clone()),
            ..settings()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        ab.status().settings.receive_directory,
        Some(receive.clone())
    );
    let a = Clipboard::new();
    let (sa, aa) = start(&a).await;
    pair(&aa, &ab).await;
    let source = directory.path().join("payload.txt");
    std::fs::write(&source, b"uses host default").unwrap();
    aa.send_files(vec![source]).await.unwrap();
    wait(&ab, |s| {
        s.content_transfers
            .iter()
            .any(|t| t.stage == ContentStage::Completed)
    })
    .await;
    let saved = &ab.snapshot().content_transfers[0].saved_paths[0];
    assert!(saved.starts_with(std::fs::canonicalize(receive).unwrap()));
    assert_eq!(std::fs::read(saved).unwrap(), b"uses host default");
    sa.shutdown().await.unwrap();
    sb.shutdown().await.unwrap();
}
