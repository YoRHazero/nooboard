use super::{LocalAddress, NearbyDevice, mdns::Discovery};
use crate::error::{Failure, InternalResult as Result};
use std::net::SocketAddr;
use tokio::sync::{mpsc, oneshot, watch};

pub(crate) enum Request {
    Addresses {
        reply: oneshot::Sender<Result<Vec<LocalAddress>>>,
    },
    Refresh {
        reply: oneshot::Sender<Result<()>>,
    },
    Enable {
        enabled: bool,
        reply: oneshot::Sender<Result<()>>,
    },
}
pub(crate) struct Handle {
    pub requests: mpsc::Sender<Request>,
    pub status: watch::Receiver<Vec<NearbyDevice>>,
}
pub(crate) struct Runtime {
    driver: Option<Discovery>,
    own_id: String,
    name: String,
    pairing: SocketAddr,
    sync: SocketAddr,
    requests: mpsc::Receiver<Request>,
    status: watch::Sender<Vec<NearbyDevice>>,
    stop: watch::Receiver<bool>,
    observed: watch::Receiver<Vec<NearbyDevice>>,
}
impl Runtime {
    pub fn new(
        own_id: String,
        name: String,
        pairing: SocketAddr,
        sync: SocketAddr,
        enabled: bool,
        capacity: usize,
        stop: watch::Receiver<bool>,
    ) -> Result<(Self, Handle)> {
        let (tx, requests) = mpsc::channel(capacity);
        let (status, rx) = watch::channel(vec![]);
        let (_, observed) = watch::channel(vec![]);
        let mut runtime = Self {
            driver: None,
            own_id,
            name,
            pairing,
            sync,
            requests,
            status,
            stop,
            observed,
        };
        if enabled {
            runtime.enable()?;
        }
        Ok((
            runtime,
            Handle {
                requests: tx,
                status: rx,
            },
        ))
    }
    fn enable(&mut self) -> Result<()> {
        if self.driver.is_none() {
            let mut driver = Discovery::new(self.own_id.clone()).map_err(|_| Failure::Internal)?;
            driver
                .advertise(&self.name, self.pairing, self.sync.port(), true)
                .map_err(|_| Failure::Internal)?;
            self.observed = driver.subscribe();
            self.driver = Some(driver);
        }
        Ok(())
    }
    async fn disable(&mut self) -> Result<()> {
        let result = if let Some(driver) = self.driver.take() {
            driver.shutdown().await.map_err(|_| Failure::Internal)
        } else {
            Ok(())
        };
        self.status.send_replace(vec![]);
        result
    }
    pub async fn run(mut self) -> Result<()> {
        loop {
            if *self.stop.borrow() {
                break;
            }
            tokio::select! {
                _ = self.stop.changed() => break,
                request = self.requests.recv() => match request {
                    Some(Request::Addresses { reply }) => {
                        if !reply.is_closed() {
                            let pairing = self.pairing;
                            let sync = self.sync;
                            let result = tokio::task::spawn_blocking(move || super::pairing_addresses(pairing, sync))
                                .await.map_err(|_| Failure::Internal)
                                .and_then(|r| r.map_err(Failure::from));
                            let _ = reply.send(result);
                        }
                    },
                    Some(Request::Refresh { reply }) => {
                        if !reply.is_closed() {
                            let result = self.driver.as_mut()
                                .ok_or(Failure::InvalidArgument("discovery disabled"))
                                .and_then(|d| d.refresh().map_err(|_| Failure::Internal));
                            let _ = reply.send(result);
                        }
                    },
                    Some(Request::Enable { enabled, reply }) => {
                        if !reply.is_closed() {
                            let result = if enabled { self.enable() } else { self.disable().await };
                            let _ = reply.send(result);
                        }
                    },
                    None => break,
                },
                changed = self.observed.changed(), if self.driver.is_some() => {
                    changed.map_err(|_| Failure::Internal)?;
                    self.status.send_replace(self.observed.borrow_and_update().clone());
                },
            }
        }
        self.requests.close();
        self.disable().await
    }
}
