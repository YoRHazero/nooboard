use crate::{Endpoint, Error, Result};
use nooboard_network::{Connection, Message, TlsConfig};
use std::time::Duration;
use tokio::{
    net::TcpListener,
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

pub(crate) struct Outbound {
    pub message: Message,
    pub reply: oneshot::Sender<nooboard_network::Result<()>>,
}
pub(crate) type Generation = (u64, u64);
pub(crate) enum LinkEvent {
    Online {
        generation: Generation,
        sender: mpsc::Sender<Outbound>,
    },
    Offline {
        generation: Generation,
    },
    Message {
        generation: Generation,
        message: Message,
    },
    Fault {
        instance: u64,
        message: String,
    },
}
pub(crate) fn start(
    instance: u64,
    endpoint: Endpoint,
    tls: TlsConfig,
    events: mpsc::Sender<LinkEvent>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut attempt = 0_u64;
        let mut listener = None;
        loop {
            attempt = match attempt.checked_add(1) {
                Some(n) => n,
                None => return,
            };
            let generation = (instance, attempt);
            let connection = match &endpoint {
                Endpoint::Connect(address) => tls.connect(address).await,
                Endpoint::Listen(address) => {
                    if listener.is_none() {
                        match TcpListener::bind(address).await {
                            Ok(bound) => listener = Some(bound),
                            Err(error) => {
                                if events
                                    .send(LinkEvent::Fault {
                                        instance,
                                        message: error.to_string(),
                                    })
                                    .await
                                    .is_err()
                                {
                                    return;
                                }
                                tokio::time::sleep(Duration::from_secs(1)).await;
                                continue;
                            }
                        }
                    }
                    match listener
                        .as_ref()
                        .expect("listener was bound")
                        .accept()
                        .await
                    {
                        Ok((tcp, _)) => tls.accept(tcp).await,
                        Err(error) => Err(error.into()),
                    }
                }
            };
            match connection {
                Ok(connection) => {
                    let (sender, outbound) = mpsc::channel(8);
                    if events
                        .send(LinkEvent::Online { generation, sender })
                        .await
                        .is_err()
                    {
                        return;
                    }
                    let _ = drive(connection, outbound, generation, &events).await;
                    if events
                        .send(LinkEvent::Offline { generation })
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
                Err(error) => {
                    if events
                        .send(LinkEvent::Fault {
                            instance,
                            message: error.to_string(),
                        })
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    })
}
async fn drive(
    mut connection: Connection,
    mut outgoing: mpsc::Receiver<Outbound>,
    generation: Generation,
    events: &mpsc::Sender<LinkEvent>,
) -> Result<()> {
    let mut heartbeat = tokio::time::interval(Duration::from_secs(5));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            item = outgoing.recv() => {
                let item = item.ok_or(Error::Stopped)?;
                let result = connection.send(&item.message).await;
                let failed = result.is_err();
                let _ = item.reply.send(result);
                if failed { return Err(Error::Offline); }
            }
            message = connection.receive() => {
                let message = message?;
                match message {
                    Message::Ping => connection.send(&Message::Pong).await?,
                    Message::Pong => {},
                    // Backpressure closes the connection instead of blocking writes indefinitely.
                    message => events.try_send(LinkEvent::Message { generation, message }).map_err(|_| Error::Offline)?,
                }
            }
            _ = heartbeat.tick() => connection.send(&Message::Ping).await?,
        }
    }
}
pub(crate) async fn send(sender: &mpsc::Sender<Outbound>, message: Message) -> Result<()> {
    let (reply, response) = oneshot::channel();
    sender
        .send(Outbound { message, reply })
        .await
        .map_err(|_| Error::Offline)?;
    response.await.map_err(|_| Error::Offline)??;
    Ok(())
}
