use crate::{
    connections::transport::tls_tcp::TlsConfig,
    identity::material::Identity,
    transfer::protocol::{Message, MessageId},
};
use tokio::net::TcpListener;

#[tokio::test]
async fn mutual_authentication_and_text_delivery() {
    let a = Identity::generate().unwrap();
    let b = Identity::generate().unwrap();
    let restored = Identity::from_secret(&a.export_secret()).unwrap();
    assert_eq!(restored.fingerprint(), a.fingerprint());
    let ac = TlsConfig::new(&a, b.certificate()).unwrap();
    let bc = TlsConfig::new(&b, a.certificate()).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let server = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let mut connection = bc.accept(tcp).await.unwrap();
        let message = connection.receive().await.unwrap();
        connection.send(&message).await.unwrap();
    });
    let mut client = ac.connect(&address).await.unwrap();
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
    let a = Identity::generate().unwrap();
    let b = Identity::generate().unwrap();
    let stranger = Identity::generate().unwrap();
    let ac = TlsConfig::new(&a, b.certificate()).unwrap();
    let bc = TlsConfig::new(&b, stranger.certificate()).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let server = tokio::spawn(async move { bc.accept(listener.accept().await.unwrap().0).await });
    assert!(ac.connect(&address).await.is_err());
    assert!(server.await.unwrap().is_err());
}

#[tokio::test]
async fn client_rejects_an_unconfirmed_server_certificate() {
    let a = Identity::generate().unwrap();
    let b = Identity::generate().unwrap();
    let expected = Identity::generate().unwrap();
    let ac = TlsConfig::new(&a, expected.certificate()).unwrap();
    let bc = TlsConfig::new(&b, a.certificate()).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let server = tokio::spawn(async move { bc.accept(listener.accept().await.unwrap().0).await });
    assert!(ac.connect(&address).await.is_err());
    assert!(server.await.unwrap().is_err());
}

#[test]
fn noob_id_survives_certificate_reissue_and_secret_restore() {
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
        crate::identity::material::fingerprint(first.der()),
        crate::identity::material::fingerprint(second.der())
    );
    assert_eq!(
        crate::identity::material::noob_id(first.der()).unwrap(),
        crate::identity::material::noob_id(second.der()).unwrap()
    );
    let identity = Identity::generate().unwrap();
    assert_eq!(identity.noob_id().unwrap().len(), 64);
    assert_eq!(
        identity.noob_id().unwrap(),
        Identity::from_secret(&identity.export_secret())
            .unwrap()
            .noob_id()
            .unwrap()
    );
    assert_ne!(
        identity.noob_id().unwrap(),
        Identity::generate().unwrap().noob_id().unwrap()
    );
    assert_ne!(
        crate::identity::material::new_session_id().unwrap(),
        crate::identity::material::new_session_id().unwrap()
    );
    assert!(crate::identity::material::noob_id(b"not a certificate").is_err());
}
#[tokio::test]
async fn one_listener_authenticates_two_different_peers_concurrently() {
    let server = Identity::generate().unwrap();
    let a = Identity::generate().unwrap();
    let b = Identity::generate().unwrap();
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
