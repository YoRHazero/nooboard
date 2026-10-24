use crate::{
    connections::transport::tls_tcp::TlsConfig,
    error::Failure,
    transfer::protocol::{Message, MessageId},
};
use tokio::net::TcpListener;

#[tokio::test]
async fn mutual_authentication_and_text_delivery() {
    let a = crate::test_identity::generate();
    let b = crate::test_identity::generate();
    let ac = TlsConfig::new(&a, b.certificate()).unwrap();
    let bc = TlsConfig::new(&b, a.certificate()).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let server = tokio::spawn(async move {
        let (tcp, remote_address) = listener.accept().await.unwrap();
        let mut connection = bc.accept(tcp).await.unwrap();
        assert_eq!(connection.peer_address(), remote_address);
        let message = connection.receive().await.unwrap();
        connection.send(&message).await.unwrap();
    });
    let mut client = ac.connect(&address).await.unwrap();
    assert_eq!(client.peer_address().to_string(), address);
    let message = Message::Text {
        id: MessageId {
            session: "a".repeat(32),
            sequence: 1,
        },
        target_epoch: 1,
        text: " 中文 🦀\n ".into(),
    };
    client.send(&message).await.unwrap();
    assert_eq!(client.receive().await.unwrap(), message);
    server.await.unwrap();
}
#[tokio::test]
async fn untrusted_identity_cannot_establish_a_business_connection() {
    let a = crate::test_identity::generate();
    let b = crate::test_identity::generate();
    let stranger = crate::test_identity::generate();
    let ac = TlsConfig::new(&a, b.certificate()).unwrap();
    let bc = TlsConfig::new(&b, stranger.certificate()).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let server = tokio::spawn(async move { bc.accept(listener.accept().await.unwrap().0).await });
    assert!(matches!(ac.connect(&address).await, Err(Failure::Unpaired)));
    assert!(matches!(server.await.unwrap(), Err(Failure::Unpaired)));
}

#[tokio::test]
async fn client_rejects_an_unconfirmed_server_certificate() {
    let a = crate::test_identity::generate();
    let b = crate::test_identity::generate();
    let expected = crate::test_identity::generate();
    let ac = TlsConfig::new(&a, expected.certificate()).unwrap();
    let bc = TlsConfig::new(&b, a.certificate()).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let server = tokio::spawn(async move { bc.accept(listener.accept().await.unwrap().0).await });
    assert!(ac.connect(&address).await.is_err());
    assert!(server.await.unwrap().is_err());
}

#[test]
fn noob_id_survives_certificate_reissue() {
    let key = rcgen::KeyPair::generate().unwrap();
    let first = rcgen::CertificateParams::new(vec!["nooboard.local".into()])
        .unwrap()
        .self_signed(&key)
        .unwrap();
    let second = rcgen::CertificateParams::new(vec!["another-name.local".into()])
        .unwrap()
        .self_signed(&key)
        .unwrap();
    assert_ne!(
        crate::peers::certificate::fingerprint(first.der()),
        crate::peers::certificate::fingerprint(second.der())
    );
    assert_eq!(
        crate::peers::certificate::noob_id(first.der()).unwrap(),
        crate::peers::certificate::noob_id(second.der()).unwrap()
    );
    let identity = crate::test_identity::generate();
    assert_eq!(identity.noob_id().unwrap().len(), 64);
    assert_ne!(
        identity.noob_id().unwrap(),
        crate::test_identity::generate().noob_id().unwrap()
    );
    assert_ne!(
        crate::random::new_session_id().unwrap(),
        crate::random::new_session_id().unwrap()
    );
    assert!(crate::peers::certificate::noob_id(b"not a certificate").is_err());
}
#[tokio::test]
async fn one_listener_authenticates_two_different_peers_concurrently() {
    let server = crate::test_identity::generate();
    let a = crate::test_identity::generate();
    let b = crate::test_identity::generate();
    let trust = TlsConfig::with_peers(
        &server,
        &[a.certificate().to_vec(), b.certificate().to_vec()],
    )
    .unwrap();
    let expected = [a.noob_id().unwrap(), b.noob_id().unwrap()]
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let accept = tokio::spawn(async move {
        let mut jobs = tokio::task::JoinSet::new();
        for _ in 0..2 {
            let tcp = listener.accept().await.unwrap().0;
            let trust = trust.clone();
            jobs.spawn(async move { trust.accept(tcp).await.unwrap() });
        }
        let mut ids = std::collections::BTreeSet::new();
        while let Some(result) = jobs.join_next().await {
            ids.insert(result.unwrap().peer_id().to_owned());
        }
        ids
    });
    let ac = TlsConfig::new(&a, server.certificate()).unwrap();
    let bc = TlsConfig::new(&b, server.certificate()).unwrap();
    let (one, two) = tokio::join!(ac.connect(&address), bc.connect(&address));
    assert_eq!(one.unwrap().peer_id(), server.noob_id().unwrap());
    assert_eq!(two.unwrap().peer_id(), server.noob_id().unwrap());
    assert_eq!(accept.await.unwrap(), expected);
}

#[tokio::test]
async fn removing_the_last_peer_still_returns_an_authenticated_unpaired_receipt() {
    let a = crate::test_identity::generate();
    let b = crate::test_identity::generate();
    let ac = TlsConfig::new(&a, b.certificate()).unwrap();
    let bc = TlsConfig::with_peers(&b, &[]).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let server = tokio::spawn(async move { bc.accept(listener.accept().await.unwrap().0).await });
    assert!(matches!(ac.connect(&address).await, Err(Failure::Unpaired)));
    assert!(matches!(server.await.unwrap(), Err(Failure::Unpaired)));
}

#[tokio::test]
async fn another_trusted_server_cannot_revoke_the_expected_peer() {
    let client = crate::test_identity::generate();
    let expected = crate::test_identity::generate();
    let other = crate::test_identity::generate();
    let cc = TlsConfig::with_peers(
        &client,
        &[
            expected.certificate().to_vec(),
            other.certificate().to_vec(),
        ],
    )
    .unwrap();
    let server = TlsConfig::with_peers(&other, &[]).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let accept =
        tokio::spawn(async move { server.accept(listener.accept().await.unwrap().0).await });
    assert!(matches!(
        cc.connect_peer(&address, &expected.noob_id().unwrap())
            .await,
        Err(Failure::Identity)
    ));
    assert!(accept.await.unwrap().is_err());
}
