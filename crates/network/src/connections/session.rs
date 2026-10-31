use super::{
    queue::Outbox,
    runtime::SessionEvent,
    transport::tls_tcp::{Connection, ConnectionReceiver, ConnectionSender},
};
use crate::{
    error::{Failure, InternalResult as Result},
    transfer::protocol::Message,
};
use std::time::Duration;
use tokio::sync::mpsc;

pub(super) async fn run(
    connection: Connection,
    queue: Outbox,
    peer: String,
    generation: u64,
    events: mpsc::Sender<SessionEvent>,
) -> Result<()> {
    struct Close(Outbox);
    impl Drop for Close {
        fn drop(&mut self) {
            self.0.close();
        }
    }
    let _close = Close(queue.clone());
    let (mut sender, mut receiver) = connection.into_split();
    let result = tokio::select! {
        result = write(&mut sender, &queue, &peer, generation, &events) => result,
        result = read(&mut receiver, &queue, &peer, generation, &events) => result,
        _ = queue.closed() => Err(Failure::Disconnected),
    };
    receiver.close().await;
    result
}
async fn write(
    sender: &mut ConnectionSender,
    queue: &Outbox,
    peer: &str,
    generation: u64,
    events: &mpsc::Sender<SessionEvent>,
) -> Result<()> {
    let mut heartbeat = tokio::time::interval(Duration::from_secs(5));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            job = queue.next() => {
                let job = job.ok_or(Failure::Disconnected)?;
                if let Some(id) = job.id() {
                    events.try_send(SessionEvent::Started {
                        peer: peer.into(), generation, id: id.clone(),
                    }).map_err(|_| Failure::Busy)?;
                }
                sender.send(&job.message).await?;
                if matches!(job.message, Message::Unpaired) {
                    return Ok(());
                }
                if let Some(id) = job.id() {
                    events.try_send(SessionEvent::Written {
                        peer: peer.into(), generation, id: id.clone(),
                    }).map_err(|_| Failure::Busy)?;
                }
            },
            _ = heartbeat.tick() => sender.send(&Message::Ping).await?,
        }
    }
}
async fn read(
    receiver: &mut ConnectionReceiver,
    queue: &Outbox,
    peer: &str,
    generation: u64,
    events: &mpsc::Sender<SessionEvent>,
) -> Result<()> {
    loop {
        match receiver.receive().await? {
            Message::Unpaired => return Err(Failure::Unpaired),
            Message::Ping => match queue.control(Message::Pong) {
                // Revocation reserves the outbox for Unpaired. A late heartbeat
                // must not end this reader and cancel the pending final write.
                Ok(()) | Err(Failure::Disconnected) => {}
                Err(error) => return Err(error),
            },
            Message::Pong => {}
            message => events
                .try_send(SessionEvent::Message {
                    peer: peer.into(),
                    generation,
                    message,
                })
                .map_err(|_| Failure::Busy)?,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connections::transport::tls_tcp::TlsConfig;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn heartbeat_during_revocation_cannot_abort_the_final_notice() {
        let a = crate::test_identity::generate();
        let b = crate::test_identity::generate();
        let client = TlsConfig::new(&a, b.certificate()).unwrap();
        let server = TlsConfig::new(&b, a.certificate()).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let (client, server) = tokio::join!(client.connect(&address), async {
            server.accept(listener.accept().await.unwrap().0).await
        });
        let (mut peer, mut peer_receiver) = client.unwrap().into_split();
        let (_sender, mut receiver) = server.unwrap().into_split();
        let queue = Outbox::new();
        queue.revoke();
        // A heartbeat may already be in flight when local revocation starts.
        peer.send(&Message::Ping).await.unwrap();
        peer.send(&Message::Unpaired).await.unwrap();
        let (events, _events) = mpsc::channel(1);
        let result = tokio::time::timeout(
            Duration::from_secs(3),
            read(&mut receiver, &queue, "peer", 1, &events),
        )
        .await
        .unwrap();
        assert!(matches!(result, Err(Failure::Unpaired)), "{result:?}");
        assert!(matches!(
            queue.next().await.unwrap().message,
            Message::Unpaired
        ));
        receiver.close().await;
        peer_receiver.close().await;
    }
}
