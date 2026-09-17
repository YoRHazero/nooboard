use super::*;

#[tokio::test]
async fn default_receive_directory_initializes_new_and_unset_profiles_without_creating_folders() {
    use crate::{devices::Configuration, ports::Store};

    for saved_without_directory in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let database = root.path().join("settings.db");
        let directory = root.path().join("下载 自定义位置").join("Nooboard");
        let identity = Identity::generate().unwrap();
        let store = Store::new(Database::open(&database).unwrap());
        if saved_without_directory {
            let previous = Configuration::load(&store, &identity, None).await.unwrap();
            assert!(previous.settings.receive_directory.is_none());
        }
        let initial = Configuration::load(&store, &identity, Some(directory.clone()))
            .await
            .unwrap();
        assert_eq!(initial.settings.receive_directory, Some(directory.clone()));
        assert!(!directory.parent().unwrap().exists());
        drop(store);

        let store = Store::new(Database::open(&database).unwrap());
        let restarted = Configuration::load(&store, &identity, Some(root.path().join("other")))
            .await
            .unwrap();
        assert_eq!(restarted.settings.receive_directory, Some(directory));
    }
}

#[tokio::test]
async fn default_receive_directory_preserves_custom_settings_even_when_system_lookup_fails() {
    use crate::{devices::Configuration, ports::Store};

    let root = tempfile::tempdir().unwrap();
    let custom = root.path().join("my files");
    let mut database = Database::in_memory().unwrap();
    database
        .set_setting(
            "settings",
            &serde_json::to_vec(&Settings {
                receive_directory: Some(custom.clone()),
                ..Settings::default()
            })
            .unwrap(),
        )
        .unwrap();
    let store = Store::new(database);
    let identity = Identity::generate().unwrap();
    for default in [Some(root.path().join("Nooboard")), None] {
        let loaded = Configuration::load(&store, &identity, default)
            .await
            .unwrap();
        assert_eq!(loaded.settings.receive_directory, Some(custom.clone()));
    }
    assert!(!custom.exists());
}

#[tokio::test]
async fn identity_confirmation_and_multiple_pairing_are_enforced() {
    let a = app(&FakeClipboard::new()).await;
    let b = app(&FakeClipboard::new()).await;
    let mut bad = request(&b);
    bad.confirmed_fingerprint = "wrong".into();
    assert!(matches!(a.trust_peer(bad).await, Err(Error::Fingerprint)));
    assert!(matches!(
        a.trust_peer(request(&a)).await,
        Err(Error::Configuration)
    ));
    a.trust_peer(request(&b)).await.unwrap();
    let c = app(&FakeClipboard::new()).await;
    a.trust_peer(request(&c)).await.unwrap();
    assert_eq!(a.status().peers.len(), 2);
    assert!(a.status().peers.iter().all(|p| !p.settings.auto_send));
    a.shutdown().await.unwrap();
    b.shutdown().await.unwrap();
    c.shutdown().await.unwrap();
}
#[tokio::test]
async fn legacy_pair_migrates_atomically_and_multi_device_settings_survive_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.db");
    let identity = Identity::generate().unwrap();
    let secret = identity.export_secret();
    let peer = Identity::generate().unwrap();
    let peer_id = peer.noob_id().unwrap();
    let mut db = Database::open(&path).unwrap();
    let mut migrated: Settings = serde_json::from_slice(br#"{"mode":"Automatic","receive":true,"paused":false,"history":true,"max_history_entries":1000,"history_days":30,"listen_address":"127.0.0.1:0"}"#).unwrap();
    assert_eq!(migrated.pairing_listen_address, "0.0.0.0:24817");
    assert!(migrated.discoverable);
    migrated.pairing_listen_address = "127.0.0.1:0".into();
    migrated.discoverable = false;
    db.set_setting("settings", &serde_json::to_vec(&migrated).unwrap())
        .unwrap();
    db.set_setting("peer", &serde_json::to_vec(&serde_json::json!({"certificate":peer.certificate(),"endpoint":{"Connect":"127.0.0.1:1"}})).unwrap()).unwrap();
    db.record_text("existing history", "local", i64::MAX, 1000, 0)
        .unwrap();
    let a = bootstrap::start_parts(db, identity, Box::new(FakeClipboard::new()), None)
        .await
        .unwrap();
    let own_id = a.status().noob_id;
    assert_eq!(a.status().peers[0].noob_id, peer_id);
    assert!(a.status().peers[0].settings.auto_send);
    assert_eq!(a.status().manual_targets, vec![peer_id.clone()]);
    let b = app(&FakeClipboard::new()).await;
    a.trust_peer(request(&b)).await.unwrap();
    a.select_targets(vec![b.status().noob_id]).await.unwrap();
    a.set_settings(Settings {
        device_name: "新的名字".into(),
        ..a.status().settings
    })
    .await
    .unwrap();
    a.shutdown().await.unwrap();
    let db = Database::open(&path).unwrap();
    assert!(db.setting("peer").unwrap().is_none());
    assert!(db.setting("settings").unwrap().is_none());
    let a = bootstrap::start_parts(
        db,
        Identity::from_secret(&secret).unwrap(),
        Box::new(FakeClipboard::new()),
        None,
    )
    .await
    .unwrap();
    assert_eq!(a.status().noob_id, own_id);
    assert_eq!(a.status().settings.device_name, "新的名字");
    assert_eq!(a.status().peers.len(), 2);
    assert_eq!(a.status().manual_targets, vec![b.status().noob_id]);
    assert_eq!(
        a.history("".into(), 100, 0).await.unwrap()[0].text,
        "existing history"
    );
    a.unpair(peer_id).await.unwrap();
    a.shutdown().await.unwrap();
    let a = bootstrap::start_parts(
        Database::open(&path).unwrap(),
        Identity::from_secret(&secret).unwrap(),
        Box::new(FakeClipboard::new()),
        None,
    )
    .await
    .unwrap();
    assert_eq!(a.status().peers.len(), 1);
    a.shutdown().await.unwrap();
    b.shutdown().await.unwrap();
}
