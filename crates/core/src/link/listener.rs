use super::LinkEvent;
use crate::Result;
use nooboard_network::TlsConfig;
use std::time::Duration;
use tokio::{
    net::TcpListener,
    sync::{mpsc, watch},
    task::{JoinHandle, JoinSet},
};

pub(crate) struct Listener {
    pub address: String,
    pub trust: watch::Sender<Option<TlsConfig>>,
    task: JoinHandle<()>,
}
impl Listener {
    pub async fn bind(
        address: &str,
        tls: Option<TlsConfig>,
        events: mpsc::Sender<LinkEvent>,
    ) -> Result<Self> {
        let listener = TcpListener::bind(address)
            .await
            .map_err(nooboard_network::Error::from)?;
        let address = listener
            .local_addr()
            .map_err(nooboard_network::Error::from)?
            .to_string();
        let (trust, config) = watch::channel(tls);
        let task = tokio::spawn(async move {
            let mut handshakes = JoinSet::new();
            loop {
                tokio::select! {
                    incoming = listener.accept() => {
                        let (tcp, _) = match incoming { Ok(value) => value, Err(error) => {
                            let _ = events.try_send(LinkEvent::Fault { peer: None, message: error.to_string() });
                            tokio::time::sleep(Duration::from_secs(1)).await;
                            continue;
                        }};
                        let tls = config.borrow().clone();
                        if let Some(tls) = tls.filter(|_| handshakes.len() < 32) {
                            let events = events.clone();
                            handshakes.spawn(async move {
                                if let Ok(connection) = tls.accept(tcp).await {
                                    let _ = events.send(LinkEvent::Connected { connection, initiated: false, dial_generation: None }).await;
                                }
                            });
                        }
                    }
                    _ = handshakes.join_next(), if !handshakes.is_empty() => {}
                }
            }
        });
        Ok(Self {
            address,
            trust,
            task,
        })
    }
}
impl Drop for Listener {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub(crate) struct Dial {
    task: JoinHandle<()>,
}
impl Dial {
    pub fn start(
        peer: String,
        addresses: Vec<String>,
        generation: u64,
        tls: TlsConfig,
        mut online: watch::Receiver<bool>,
        events: mpsc::Sender<LinkEvent>,
    ) -> Self {
        let task = tokio::spawn(async move {
            let mut attempt = 0usize;
            loop {
                while *online.borrow_and_update() {
                    if online.changed().await.is_err() {
                        return;
                    }
                }
                let address = &addresses[attempt % addresses.len()];
                attempt = attempt.wrapping_add(1);
                let result = tls.connect_peer(address, &peer).await;
                if let Ok(connection) = result
                    && events
                        .send(LinkEvent::Connected {
                            connection,
                            initiated: true,
                            dial_generation: Some(generation),
                        })
                        .await
                        .is_err()
                {
                    return;
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
                if online.has_changed().is_err() {
                    return;
                }
            }
        });
        Self { task }
    }
}
impl Drop for Dial {
    fn drop(&mut self) {
        self.task.abort();
    }
}
