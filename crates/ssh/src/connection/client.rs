use crate::{
    runtime::{self, Context},
    *,
};
use russh::{
    client,
    keys::{HashAlg, PublicKeyOrCertificate},
};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::{
    net::TcpStream,
    sync::{mpsc, oneshot},
    time::timeout,
};

#[derive(Clone)]
pub(crate) struct KeyDecisions {
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<bool>>>>,
    events: mpsc::Sender<Event>,
}
impl KeyDecisions {
    pub fn new(events: mpsc::Sender<Event>) -> Self {
        Self {
            pending: Arc::default(),
            events,
        }
    }
    pub fn decide(&self, id: &str, accept: bool) -> Result<()> {
        self.pending
            .lock()
            .map_err(|_| Error::Internal)?
            .remove(id)
            .ok_or(Error::NotFound)?
            .send(accept)
            .map_err(|_| Error::Cancelled)
    }
    pub fn clear(&self) {
        if let Ok(mut pending) = self.pending.lock() {
            pending.clear();
        }
    }
}
pub(crate) struct Handler {
    context: Context,
    session: String,
    address: SocketAddr,
    expected: Option<HostKey>,
    accepted: Arc<Mutex<Option<HostKey>>>,
    failure: Arc<Mutex<Option<Error>>>,
}
impl client::Handler for Handler {
    type Error = russh::Error;
    async fn check_server_key(
        &mut self,
        key: &PublicKeyOrCertificate,
    ) -> std::result::Result<bool, Self::Error> {
        let key = match key {
            PublicKeyOrCertificate::PublicKey { key, .. } => HostKey {
                algorithm: key.algorithm().to_string(),
                fingerprint: key.fingerprint(HashAlg::Sha256).to_string(),
                public_key: key.to_openssh().map_err(|_| russh::Error::UnknownKey)?,
            },
            // Certificate authority trust needs a separate product policy.
            _ => return Ok(false),
        };
        let result = if let Some(expected) = &self.expected {
            if expected.public_key == key.public_key {
                Ok(())
            } else {
                Err(Error::HostKeyChanged)
            }
        } else {
            let id = runtime::id();
            let (tx, rx) = oneshot::channel();
            self.context
                .keys
                .pending
                .lock()
                .expect("key decisions poisoned")
                .insert(id.clone(), tx);
            let mut stop = self.context.stop.clone();
            let event = Event::HostKeyRequired {
                request_id: id.clone(),
                session_id: self.session.clone(),
                address: self.address,
                key: key.clone(),
            };
            let approval = async {
                self.context
                    .keys
                    .events
                    .send(event)
                    .await
                    .map_err(|_| Error::HostKeyRejected)?;
                if rx.await.unwrap_or(false) {
                    Ok(())
                } else {
                    Err(Error::HostKeyRejected)
                }
            };
            let result = tokio::select! {
                _ = stop.changed() => Err(Error::Stopped),
                result = timeout(self.context.options.host_key_timeout, approval) => result.unwrap_or(Err(Error::Timeout)),
            };
            self.context
                .keys
                .pending
                .lock()
                .expect("key decisions poisoned")
                .remove(&id);
            result
        };
        match result {
            Ok(()) => {
                *self.accepted.lock().expect("accepted key poisoned") = Some(key);
                Ok(true)
            }
            Err(e) => {
                *self.failure.lock().expect("key failure poisoned") = Some(e);
                Ok(false)
            }
        }
    }
}
pub(crate) type Connection = client::Handle<Handler>;

/// russh spawns its transport before key exchange completes. Closing a duplicate
/// socket on cancellation also interrupts that task while it is still in KEX.
struct AttemptSocket(Option<std::net::TcpStream>);
impl Drop for AttemptSocket {
    fn drop(&mut self) {
        if let Some(socket) = &self.0 {
            let _ = socket.shutdown(std::net::Shutdown::Both);
        }
    }
}
pub(crate) async fn connect(
    context: &Context,
    session: &str,
    addresses: &[SocketAddr],
    username: &str,
    password: &Password,
    expected: Option<HostKey>,
) -> Result<(Connection, HostKey, SocketAddr)> {
    let mut stop = context.stop.clone();
    if *stop.borrow() {
        return Err(Error::Stopped);
    }
    let operation = async {
        let mut connected = None;
        let mut error = Error::NotFound;
        for address in addresses {
            match timeout(context.options.connect_timeout, TcpStream::connect(address)).await {
                Ok(Ok(stream)) => {
                    connected = Some((stream, *address));
                    break;
                }
                Ok(Err(e)) => error = Error::Connection(e.to_string()),
                Err(_) => error = Error::Timeout,
            }
        }
        let (stream, address) = connected.ok_or(error)?;
        let stream = stream.into_std()?;
        let mut interrupt = AttemptSocket(Some(stream.try_clone()?));
        let stream = TcpStream::from_std(stream)?;
        let handshake_timeout = context.options.connect_timeout
            + if expected.is_none() {
                context.options.host_key_timeout
            } else {
                std::time::Duration::ZERO
            };
        let accepted = Arc::new(Mutex::new(None));
        let failure = Arc::new(Mutex::new(None));
        let handler = Handler {
            context: context.clone(),
            session: session.into(),
            address,
            expected,
            accepted: accepted.clone(),
            failure: failure.clone(),
        };
        let config = Arc::new(client::Config {
            keepalive_interval: Some(std::time::Duration::from_secs(15)),
            keepalive_max: 3,
            ..Default::default()
        });
        let connection = timeout(
            handshake_timeout,
            client::connect_stream(config, stream, handler),
        )
        .await
        .map_err(|_| Error::Timeout)?;
        let mut connection = connection.map_err(|e| {
            failure
                .lock()
                .expect("key failure poisoned")
                .clone()
                .unwrap_or_else(|| e.into())
        })?;
        let auth = timeout(
            context.options.connect_timeout,
            connection.authenticate_password(username, password.expose()),
        )
        .await;
        match auth {
            Ok(Ok(result)) if result.success() => {}
            other => {
                let _ = timeout(
                    context.options.request_timeout,
                    connection.disconnect(
                        russh::Disconnect::ByApplication,
                        "authentication failed",
                        "en",
                    ),
                )
                .await;
                return Err(match other {
                    Err(_) => Error::Timeout,
                    Ok(Err(e)) => e.into(),
                    _ => Error::Authentication,
                });
            }
        }
        let key = accepted
            .lock()
            .map_err(|_| Error::Internal)?
            .clone()
            .ok_or(Error::HostKeyRejected)?;
        // The terminal/file runtime now owns the established connection.
        interrupt.0.take();
        Ok((connection, key, address))
    };
    tokio::select! { biased; _ = stop.changed() => Err(Error::Stopped), result = operation => result }
}
