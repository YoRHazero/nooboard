use nooboard_network::{Identity, Message, TlsConfig};
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
        sequence: 1,
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
