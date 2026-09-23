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
) {
    struct Close(Outbox);
    impl Drop for Close {
        fn drop(&mut self) {
            self.0.close();
        }
    }
    let _close = Close(queue.clone());
    let (mut sender, mut receiver) = connection.into_split();
    tokio::select! {
        _ = write(&mut sender, &queue, &peer, generation, &events) => {},
        _ = read(&mut receiver, &queue, &peer, generation, &events) => {},
        _ = queue.closed() => {},
    }
    receiver.close().await;
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
            Message::Ping => queue.control(Message::Pong)?,
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
