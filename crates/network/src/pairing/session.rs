use super::{
    Action, Contact, Control, Error, Event, Result, Stage,
    crypto::{self, Cipher},
    wire::{self, Hello, Packet},
};
use spake2::{Ed25519Group, Identity, Password, Spake2};
use std::{net::SocketAddr, time::Duration};
use tokio::{
    net::TcpStream,
    sync::{mpsc, oneshot, watch},
};

struct Session {
    id: String,
    stream: TcpStream,
    peer_address: SocketAddr,
    events: mpsc::Sender<Event>,
    actions: mpsc::Receiver<Action>,
}
impl Session {
    // While waiting for local input, still notice a peer cancellation or disconnect.
    async fn action(&mut self) -> Result<Action> {
        tokio::select! {
            action = self.actions.recv() => action.ok_or(Error::Cancelled),
            packet = wire::read(&mut self.stream) => match packet {
                Ok(Packet::Reject) => Err(Error::Rejected),
                Err(error) => Err(error),
                _ => Err(Error::Protocol),
            }
        }
    }

    async fn progress(&self, stage: Stage, code: Option<String>, attempts_left: u8) -> Result<()> {
        self.events
            .send(Event::Progress {
                id: self.id.clone(),
                stage,
                code,
                attempts_left,
            })
            .await
            .map_err(|_| Error::Cancelled)
    }
    async fn offered(&self, peer: Contact, control: Control, incoming: bool) -> Result<()> {
        self.events
            .send(Event::Offered {
                id: self.id.clone(),
                peer,
                control,
                incoming,
            })
            .await
            .map_err(|_| Error::Cancelled)
    }
    async fn save(&self, peer: Contact) -> Result<()> {
        let mut address = self.peer_address;
        address.set_port(peer.sync_port);
        let address = address.to_string();
        let (saved, receive) = oneshot::channel();
        self.events
            .send(Event::Verified {
                id: self.id.clone(),
                peer,
                address,
                saved,
            })
            .await
            .map_err(|_| Error::Cancelled)?;
        match receive.await {
            Ok(true) => Ok(()),
            _ => Err(Error::Storage),
        }
    }
    async fn send_secure(&mut self, cipher: &mut Cipher, message: &[u8]) -> Result<()> {
        wire::write(&mut self.stream, &Packet::Sealed(cipher.seal(message)?)).await
    }
    async fn expect_secure(&mut self, cipher: &mut Cipher, message: &[u8]) -> Result<()> {
        let Packet::Sealed(bytes) = wire::read(&mut self.stream).await? else {
            return Err(Error::Protocol);
        };
        if cipher.open(&bytes)? != message {
            return Err(Error::Protocol);
        }
        Ok(())
    }
    async fn server(&mut self, local: Contact, control: Control) -> Result<()> {
        let Packet::Hello(a) =
            tokio::time::timeout(Duration::from_secs(5), wire::read(&mut self.stream))
                .await
                .map_err(|_| Error::Timeout)??
        else {
            return Err(Error::Protocol);
        };
        validate_hello(&a, &local)?;
        wire::write(&mut self.stream, &Packet::Waiting).await?;
        self.offered(a.contact.clone(), control, true).await?;
        if !matches!(
            tokio::time::timeout(Duration::from_secs(60), self.action())
                .await
                .map_err(|_| Error::Timeout)??,
            Action::Accept
        ) {
            return Err(Error::Protocol);
        }
        let code = super::random_code()?;
        self.progress(Stage::ShowingCode, Some(code.to_string()), 3)
            .await?;
        let b = hello(local)?;
        let id_a = wire::binding(&a)?;
        let id_b = wire::binding(&b)?;
        wire::write(&mut self.stream, &Packet::Ready(b)).await?;
        tokio::time::timeout(Duration::from_secs(120), async {
            for attempt in 0..3 {
                let Packet::Pake(bytes) = wire::read(&mut self.stream).await? else {
                    return Err(Error::Protocol);
                };
                if bytes.len() != 33 {
                    return Err(Error::Protocol);
                }
                let (state, response) = Spake2::<Ed25519Group>::start_b(
                    &Password::new(code.as_bytes()),
                    &Identity::new(&id_a),
                    &Identity::new(&id_b),
                );
                let key = state.finish(&bytes).map_err(|_| Error::Protocol)?;
                wire::write(&mut self.stream, &Packet::Pake(response)).await?;
                let (mut send, mut receive) = crypto::channels(key, false)?;
                let Packet::Sealed(proof) = wire::read(&mut self.stream).await? else {
                    return Err(Error::Protocol);
                };
                if receive.open(&proof).ok().as_deref() != Some(b"confirm-client") {
                    let left = 2 - attempt;
                    wire::write(&mut self.stream, &Packet::Retry(left)).await?;
                    if left == 0 {
                        return Err(Error::Attempts);
                    }
                    self.progress(Stage::ShowingCode, Some(code.to_string()), left)
                        .await?;
                    continue;
                }
                self.progress(Stage::Saving, None, 3 - attempt).await?;
                self.send_secure(&mut send, b"confirm-server").await?;
                self.expect_secure(&mut receive, b"ready-to-save").await?;
                self.save(a.contact.clone()).await?;
                self.send_secure(&mut send, b"server-saved").await?;
                self.expect_secure(&mut receive, b"client-saved").await?;
                self.send_secure(&mut send, b"complete").await?;
                return Ok(());
            }
            Err(Error::Attempts)
        })
        .await
        .map_err(|_| Error::Timeout)?
    }
    async fn client(&mut self, local: Contact, control: Control) -> Result<()> {
        let own = local.clone();
        let a = hello(local)?;
        let id_a = wire::binding(&a)?;
        wire::write(&mut self.stream, &Packet::Hello(a)).await?;
        match wire::read(&mut self.stream).await? {
            Packet::Waiting => {}
            _ => return Err(Error::Rejected),
        }
        self.progress(Stage::AwaitingApproval, None, 3).await?;
        let b = match wire::read(&mut self.stream).await? {
            Packet::Ready(b) => b,
            _ => return Err(Error::Rejected),
        };
        validate_hello(&b, &own)?;
        let id_b = wire::binding(&b)?;
        self.offered(b.contact.clone(), control, false).await?;
        self.progress(Stage::EnteringCode, None, 3).await?;
        tokio::time::timeout(Duration::from_secs(120), async {
            for attempt in 0..3 {
                let Action::Code(code) = self.action().await? else {
                    return Err(Error::Cancelled);
                };
                self.progress(Stage::Verifying, None, 3 - attempt).await?;
                let (state, message) = Spake2::<Ed25519Group>::start_a(
                    &Password::new(code.as_bytes()),
                    &Identity::new(&id_a),
                    &Identity::new(&id_b),
                );
                wire::write(&mut self.stream, &Packet::Pake(message)).await?;
                let Packet::Pake(response) = wire::read(&mut self.stream).await? else {
                    return Err(Error::Protocol);
                };
                if response.len() != 33 {
                    return Err(Error::Protocol);
                }
                let key = state.finish(&response).map_err(|_| Error::Protocol)?;
                let (mut send, mut receive) = crypto::channels(key, true)?;
                self.send_secure(&mut send, b"confirm-client").await?;
                match wire::read(&mut self.stream).await? {
                    Packet::Retry(left) if left == 2 - attempt && left > 0 => {
                        self.progress(Stage::EnteringCode, None, left).await?;
                        continue;
                    }
                    Packet::Retry(_) => return Err(Error::Attempts),
                    Packet::Sealed(bytes)
                        if receive.open(&bytes)?.as_slice() == b"confirm-server" => {}
                    _ => return Err(Error::Protocol),
                }
                self.progress(Stage::Saving, None, 3 - attempt).await?;
                self.send_secure(&mut send, b"ready-to-save").await?;
                self.expect_secure(&mut receive, b"server-saved").await?;
                self.save(b.contact.clone()).await?;
                self.send_secure(&mut send, b"client-saved").await?;
                self.expect_secure(&mut receive, b"complete").await?;
                return Ok(());
            }
            Err(Error::Attempts)
        })
        .await
        .map_err(|_| Error::Timeout)?
    }
}
fn hello(contact: Contact) -> Result<Hello> {
    Ok(Hello {
        version: 1,
        nonce: crate::identity::material::new_session_id().map_err(|_| Error::Protocol)?,
        contact,
    })
}
fn validate_hello(hello: &Hello, local: &Contact) -> Result<()> {
    if hello.version != 1
        || hello.nonce.len() != 32
        || !hello.nonce.bytes().all(|b| b.is_ascii_hexdigit())
        || hello.contact.validate()? == local.validate()?
    {
        return Err(Error::Protocol);
    }
    Ok(())
}
pub(super) struct Request {
    pub id: String,
    pub control: Control,
    pub actions: mpsc::Receiver<Action>,
    pub cancelled: watch::Receiver<bool>,
}
pub(super) async fn run(
    request: Request,
    stream: TcpStream,
    local: Contact,
    events: mpsc::Sender<Event>,
    client: bool,
) {
    let Request {
        id,
        control,
        actions,
        mut cancelled,
    } = request;
    let Ok(peer_address) = stream.peer_addr() else {
        return;
    };
    let _ = stream.set_nodelay(true);
    let mut session = Session {
        id: id.clone(),
        stream,
        peer_address,
        events: events.clone(),
        actions,
    };
    let already_cancelled = *cancelled.borrow();
    let result = {
        let outcome = async {
            if already_cancelled {
                return Err(Error::Cancelled);
            }
            if client {
                session.client(local, control).await
            } else {
                session.server(local, control).await
            }
        };
        tokio::select! {
            result = tokio::time::timeout(Duration::from_secs(185), outcome) => result.unwrap_or(Err(Error::Timeout)),
            _ = cancelled.changed() => Err(Error::Cancelled),
        }
    };
    if result.is_err() {
        // Best effort only; loss of the connection is also a terminal event.
        let _ = tokio::time::timeout(
            Duration::from_millis(100),
            wire::write(&mut session.stream, &Packet::Reject),
        )
        .await;
    }
    let event = match result {
        Ok(()) => Event::Completed { id },
        Err(e) => Event::Failed { id, error: e },
    };
    let _ = events.send(event).await;
}
