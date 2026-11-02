//! Kept in its own test binary: lowering RLIMIT_NOFILE affects the whole process.
#![cfg(unix)]
#[path = "support/identity.rs"]
mod identity;
use nooboard_network::*;
use std::time::Duration;

fn options(name: &str) -> Options {
    Options {
        device_name: name.into(),
        listen: "127.0.0.1:0".parse().unwrap(),
        pairing_listen: "127.0.0.1:0".parse().unwrap(),
        discovery: false,
        ..Options::default()
    }
}
async fn start(name: &str) -> (NetworkService, Network, NetworkEvents) {
    let dependencies = NetworkDependencies {
        identity: identity::generate(),
    };
    tokio::time::timeout(
        Duration::from_secs(15),
        NetworkService::start(options(name), dependencies),
    )
    .await
    .expect("start timed out")
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn descriptor_exhaustion_during_accept_does_not_stop_the_service() {
    let (a_owner, a, _a_events) = start("a").await;
    let sync = a.status().listen_address;
    let pairing = a.status().pairing_address;

    // SAFETY: getrlimit/setrlimit receive a valid, initialized rlimit.
    unsafe {
        let mut limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        assert_eq!(libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit), 0);
        limit.rlim_cur = limit.rlim_cur.min(256);
        assert_eq!(libc::setrlimit(libc::RLIMIT_NOFILE, &limit), 0);
    }
    let mut held = Vec::new();
    while let Ok(file) = std::fs::File::open("/dev/null") {
        held.push(file);
    }
    // Free exactly one descriptor per client. The kernel completes both TCP
    // handshakes, so each listener's accept() then fails with EMFILE.
    held.truncate(held.len() - 2);
    let sync_client = std::net::TcpStream::connect(sync).unwrap();
    let pairing_client = std::net::TcpStream::connect(pairing).unwrap();

    let mut status = a.subscribe();
    let stopped = tokio::time::timeout(Duration::from_millis(2500), async {
        loop {
            if status.borrow_and_update().state != ServiceState::Running {
                return;
            }
            if status.changed().await.is_err() {
                return;
            }
        }
    })
    .await;
    assert!(
        stopped.is_err(),
        "a transient accept failure stopped the service: {:?}",
        a.status().state
    );

    // Once descriptors are available again, the same listener serves real peers.
    drop((sync_client, pairing_client, held));
    let (b_owner, b, _b_events) = start("b").await;
    let b_status = b.status();
    let mut passive = TrustedPeer {
        identity: b_status.identity.clone(),
        addresses: vec![b_status.listen_address],
    };
    passive.addresses.clear();
    a.trust_peer(passive).await.unwrap();
    b.trust_peer(TrustedPeer {
        identity: a.status().identity,
        addresses: vec![sync],
    })
    .await
    .unwrap();
    let mut status = a.subscribe();
    tokio::time::timeout(Duration::from_secs(15), async {
        while !status
            .borrow_and_update()
            .connections
            .iter()
            .any(|peer| peer.peer == b_status.identity.id && peer.connected)
        {
            status.changed().await.unwrap();
        }
    })
    .await
    .expect("the sync listener did not accept a trusted peer after recovery");
    assert_eq!(a.status().state, ServiceState::Running);
    b_owner.shutdown().await.unwrap();
    a_owner.shutdown().await.unwrap();
}
