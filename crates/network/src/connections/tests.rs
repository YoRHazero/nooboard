use super::*;
use crate::identity::PublicIdentity;

#[tokio::test]
async fn revocation_blocks_dialing_until_reappearance_and_ignores_repeated_announcements() {
    let identity = Identity::generate().unwrap();
    let other = Identity::generate().unwrap();
    let id = other.noob_id().unwrap();
    let address = "127.0.0.1:9".parse().unwrap();
    let options = Options {
        listen: "127.0.0.1:0".parse().unwrap(),
        trusted_peers: vec![TrustedPeer {
            identity: PublicIdentity {
                id: id.clone(),
                fingerprint: other.fingerprint(),
                certificate: other.certificate().to_vec(),
                device_name: "other".into(),
            },
            addresses: vec![address],
        }],
        ..Options::default()
    };
    let (_stop, stopped) = watch::channel(false);
    let (mut runtime, _handle, _events) = Runtime::bind(&options, Arc::new(identity), stopped)
        .await
        .unwrap();
    let device = NearbyDevice {
        key: "other.local.".into(),
        noob_id: id.clone(),
        device_name: "other".into(),
        addresses: vec![address.to_string()],
        sync_port: 9,
    };
    runtime.refresh_discovery(std::slice::from_ref(&device));
    runtime.mark_unpaired(&id).unwrap();
    let receipt = runtime.unpaired[&id];
    let mut tasks = JoinSet::new();
    runtime.dial(&mut tasks);
    assert!(tasks.is_empty());
    runtime.refresh_discovery(std::slice::from_ref(&device));
    assert_eq!(runtime.unpaired[&id], receipt);
    runtime.dial(&mut tasks);
    assert!(tasks.is_empty());
    runtime.refresh_discovery(&[]);
    runtime.dial(&mut tasks);
    assert!(tasks.is_empty());
    runtime.refresh_discovery(&[device]);
    assert!(!runtime.unpaired.contains_key(&id));
    runtime.dial(&mut tasks);
    assert_eq!(tasks.len(), 1);
    tasks.abort_all();
}

async fn connection_state(status: &mut watch::Receiver<Vec<ConnectionStatus>>, connected: bool) {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            if status
                .borrow_and_update()
                .iter()
                .any(|p| p.connected == connected)
            {
                return;
            }
            status.changed().await.unwrap();
        }
    })
    .await
    .expect("connection did not converge");
}

#[tokio::test]
async fn late_start_and_new_discovered_address_reconnect_the_same_identity() {
    let a = Arc::new(Identity::generate().unwrap());
    let b = Arc::new(Identity::generate().unwrap());
    let a_options = Options {
        listen: "127.0.0.1:0".parse().unwrap(),
        trusted_peers: vec![TrustedPeer {
            identity: PublicIdentity::from_identity(&b, "b".into()).unwrap(),
            addresses: vec![],
        }],
        ..Options::default()
    };
    let b_options = Options {
        listen: "127.0.0.1:0".parse().unwrap(),
        trusted_peers: vec![TrustedPeer {
            identity: PublicIdentity::from_identity(&a, "a".into()).unwrap(),
            addresses: vec![],
        }],
        ..Options::default()
    };
    let (stop_a, stopped_a) = watch::channel(false);
    let (runtime_a, mut handle_a, _events_a) =
        Runtime::bind(&a_options, a, stopped_a).await.unwrap();
    let task_a = tokio::spawn(runtime_a.run());
    assert!(!handle_a.status.borrow()[0].connected);
    let mut previous_address = None;
    let mut retired = Vec::new();
    for _ in 0..2 {
        let (stop_b, stopped_b) = watch::channel(false);
        let (runtime_b, mut handle_b, _events_b) = Runtime::bind(&b_options, b.clone(), stopped_b)
            .await
            .unwrap();
        if let Some(old) = previous_address {
            assert_ne!(old, handle_b.address);
        }
        let task_b = tokio::spawn(runtime_b.run());
        // Only A learns B's current endpoint; B has no stored address to dial A.
        handle_a.hints.send_replace(vec![NearbyDevice {
            key: "b.local.".into(),
            noob_id: b.noob_id().unwrap(),
            device_name: "b".into(),
            addresses: vec![handle_b.address.to_string()],
            sync_port: handle_b.address.port(),
        }]);
        connection_state(&mut handle_a.status, true).await;
        connection_state(&mut handle_b.status, true).await;
        assert!(handle_a.status.borrow()[0].unpaired_notice.is_none());
        stop_b.send_replace(true);
        task_b.await.unwrap().unwrap();
        handle_a.hints.send_replace(vec![]);
        connection_state(&mut handle_a.status, false).await;
        // Reserve the retired endpoint so the restarted device must get a new address.
        previous_address = Some(handle_b.address);
        retired.push(TcpListener::bind(handle_b.address).await.unwrap());
    }
    stop_a.send_replace(true);
    task_a.await.unwrap().unwrap();
}
