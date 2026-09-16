use super::{LinkEvent, queue::Outbox};
use nooboard_network::{Connection, ConnectionReceiver, ConnectionSender, Message};
use std::time::Duration;
use tokio::{sync::mpsc, task::JoinHandle};

pub(crate) struct Session {
    pub outbox: Outbox,
    task: JoinHandle<()>,
}
struct Context {
    peer: String,
    generation: u64,
    events: mpsc::Sender<LinkEvent>,
}
impl Session {
    pub fn start(
        connection: Connection,
        peer: String,
        generation: u64,
        events: mpsc::Sender<LinkEvent>,
    ) -> Self {
        let outbox = Outbox::new();
        let queue = outbox.clone();
        let task = tokio::spawn(async move {
            let context = Context {
                peer,
                generation,
                events,
            };
            let (sender, receiver) = connection.into_split();
            tokio::select! {
                _ = write(sender, &queue, &context) => {},
                _ = read(receiver, &queue, &context) => {},
            }
            let _ = context
                .events
                .send(LinkEvent::Offline {
                    peer: context.peer,
                    generation,
                })
                .await;
        });
        Self { outbox, task }
    }
}
async fn write(mut sender: ConnectionSender, queue: &Outbox, context: &Context) -> Result<(), ()> {
    let mut heartbeat = tokio::time::interval(Duration::from_secs(5));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            job = queue.next() => {
                if let Some(id) = job.id() {
                    context.events.send(LinkEvent::Started { peer: context.peer.clone(), generation: context.generation, id: id.clone() }).await.map_err(|_| ())?;
                }
                sender.send(&job.message).await.map_err(|_| ())?;
                if let Some(id) = job.id() {
                    context.events.send(LinkEvent::Written { peer: context.peer.clone(), generation: context.generation, id: id.clone() }).await.map_err(|_| ())?;
                }
            }
            _ = heartbeat.tick() => sender.send(&Message::Ping).await.map_err(|_| ())?,
        }
    }
}
async fn read(
    mut receiver: ConnectionReceiver,
    queue: &Outbox,
    context: &Context,
) -> Result<(), ()> {
    loop {
        match receiver.receive().await.map_err(|_| ())? {
            Message::Ping => queue.control(Message::Pong)?,
            Message::Pong => {}
            message => {
                // Disconnect an overloaded peer rather than block all other peers.
                context
                    .events
                    .try_send(LinkEvent::Message {
                        peer: context.peer.clone(),
                        generation: context.generation,
                        message,
                    })
                    .map_err(|_| ())?;
            }
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.task.abort();
    }
}
