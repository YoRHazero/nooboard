use super::{Context, DriverEvent, Input};
use crate::*;
use russh::{
    Channel, ChannelMsg, ChannelOpenFailure, Disconnect, client,
    keys::{PrivateKeyWithHashAlg, PublicKeyOrCertificate},
};
use std::{
    net::Shutdown,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    net::TcpStream,
    sync::{OnceCell, mpsc, watch},
    time::{Instant, timeout},
};

pub(crate) struct IncomingForward {
    pub channel: Channel<client::Msg>,
    pub reply: client::ChannelOpenHandle,
    pub port: u32,
}
pub(crate) struct Client {
    verifier: Arc<dyn SshHostVerifier>,
    request: SshHostVerificationRequest,
    stop: CancellationToken,
    prompt: watch::Sender<bool>,
    trust_error: Arc<Mutex<Option<TerminalError>>>,
    ended: CancellationToken,
    incoming: Option<mpsc::Sender<IncomingForward>>,
}
impl Drop for Client {
    fn drop(&mut self) {
        self.ended.cancel();
    }
}
impl client::Handler for Client {
    type Error = russh::Error;
    async fn check_server_key(
        &mut self,
        key: &PublicKeyOrCertificate,
    ) -> std::result::Result<bool, Self::Error> {
        let bytes = match key {
            PublicKeyOrCertificate::PublicKey { key, .. } => key.to_bytes(),
            PublicKeyOrCertificate::Certificate(cert) => cert.to_bytes(),
        }
        .map_err(russh::Error::from)?;
        let algorithm = bytes
            .get(..4)
            .and_then(|n| <[u8; 4]>::try_from(n).ok())
            .map(u32::from_be_bytes)
            .and_then(|n| bytes.get(4..4 + n as usize))
            .and_then(|s| std::str::from_utf8(s).ok())
            .unwrap_or("unknown")
            .to_owned();
        let request = SshHostVerificationRequest {
            server_key: SshPublicKey { algorithm, bytes },
            ..self.request.clone()
        };
        self.prompt.send_replace(true);
        let result = tokio::select! {
            biased;
            _ = self.stop.cancelled() => Err(TerminalError::cancelled()),
            result = self.verifier.verify(request, self.stop.clone()) => result,
        };
        self.prompt.send_replace(false);
        if let Err(error) = result {
            *self.trust_error.lock().unwrap() = Some(error);
            return Ok(false);
        }
        Ok(true)
    }
    async fn server_channel_open_forwarded_tcpip(
        &mut self,
        channel: Channel<client::Msg>,
        _address: &str,
        port: u32,
        _origin: &str,
        _origin_port: u32,
        reply: client::ChannelOpenHandle,
        _session: &mut client::Session,
    ) -> std::result::Result<(), Self::Error> {
        if let Some(incoming) = &self.incoming {
            if let Err(error) = incoming.try_send(IncomingForward {
                channel,
                reply,
                port,
            }) {
                error
                    .into_inner()
                    .reply
                    .reject(ChannelOpenFailure::ResourceShortage)
                    .await;
            }
        } else {
            reply
                .reject(ChannelOpenFailure::AdministrativelyProhibited)
                .await;
        }
        Ok(())
    }
}
pub(crate) struct TransportControl {
    socket: std::net::TcpStream,
    pub ended: CancellationToken,
}
impl TransportControl {
    pub fn close(&self) {
        let _ = self.socket.shutdown(Shutdown::Both);
    }
}
impl Drop for TransportControl {
    fn drop(&mut self) {
        self.close();
    }
}
pub(crate) struct Connection {
    pub client: client::Handle<Client>,
    socket: Arc<TransportControl>,
    pub ended: CancellationToken,
    pub sftp: OnceCell<Arc<super::files::Sftp>>,
}
impl Connection {
    pub fn force_close(&self) {
        self.socket.close();
    }
    pub async fn close(&self, context: &Context) {
        tokio::select! {
            _ = context.force.cancelled() => {},
            _ = timeout(context.options.io.request_timeout, self.client.disconnect(Disconnect::ByApplication, "terminal run ended", "")) => {},
        }
        self.socket.close();
        self.ended.cancelled().await;
    }
}
pub(crate) fn error(error: impl std::fmt::Display) -> TerminalError {
    TerminalError::new(TerminalErrorKind::ConnectionFailed, error.to_string())
}
pub(crate) fn lost() -> TerminalError {
    TerminalError::new(
        TerminalErrorKind::ConnectionLost,
        "SSH transport or channel closed",
    )
}
pub(crate) async fn request<T>(
    duration: Duration,
    future: impl std::future::Future<Output = std::result::Result<T, russh::Error>>,
) -> Result<T> {
    timeout(duration, future)
        .await
        .map_err(|_| {
            TerminalError::new(TerminalErrorKind::ConnectionFailed, "SSH request timed out")
        })?
        .map_err(error)
}

pub(crate) async fn connect(
    target: &SshStartTarget,
    context: &Context,
    incoming: Option<mpsc::Sender<IncomingForward>>,
) -> Result<Arc<Connection>> {
    let options = tokio::select! {
        biased;
        _ = context.stop.cancelled() => return Err(TerminalError::cancelled()),
        result = timeout(context.options.io.connect_timeout, target.provider.prepare()) => {
            result.map_err(|_| TerminalError::new(
                TerminalErrorKind::AddressUnavailable, "address preparation timed out",
            ))??
        }
    };
    if options.username != target.summary.username || options.address.port() != target.summary.port
    {
        return Err(TerminalError::invalid(
            "connection provider changed the frozen SSH username or port",
        ));
    }
    context.event(DriverEvent::Resolved {
        id: context.id,
        address: options.address,
    });
    let stream = tokio::select! {
        biased;
        _ = context.stop.cancelled() => return Err(TerminalError::cancelled()),
        result = timeout(context.options.io.connect_timeout, TcpStream::connect(options.address)) => {
            result.map_err(|_| error("TCP connection timed out"))?.map_err(error)?
        }
    };
    let stream = stream.into_std().map_err(error)?;
    let socket = stream.try_clone().map_err(error)?;
    let stream = TcpStream::from_std(stream).map_err(error)?;
    let ended = CancellationToken::new();
    let (prompt, mut prompting) = watch::channel(false);
    let trust_error = Arc::new(Mutex::new(None));
    let handler = Client {
        verifier: context.host_verifier.clone(),
        request: SshHostVerificationRequest {
            owner_run_id: context.owner(),
            target_config_id: target.config_id,
            address: options.address,
            server_key: SshPublicKey {
                algorithm: String::new(),
                bytes: vec![],
            },
        },
        stop: context.stop.clone(),
        prompt,
        trust_error: trust_error.clone(),
        ended: ended.clone(),
        incoming,
    };
    let socket = Arc::new(TransportControl {
        socket,
        ended: ended.clone(),
    });
    *context.transport.lock().unwrap() = Some(socket.clone());
    let handshake = client::connect_stream(Arc::new(client::Config::default()), stream, handler);
    tokio::pin!(handshake);
    let mut remaining = context.options.io.connect_timeout;
    let mut since = Instant::now();
    let mut paused = false;
    let mut prompt_open = true;
    let mut session = loop {
        tokio::select! {
            biased;
            _ = context.stop.cancelled() => return Err(TerminalError::cancelled()),
            changed = prompting.changed(), if prompt_open => {
                if changed.is_err() { prompt_open = false; }
                let now = Instant::now();
                if !paused { remaining = remaining.saturating_sub(now.duration_since(since)); }
                paused = *prompting.borrow_and_update(); since = now;
            }
            result = &mut handshake => break result.map_err(|e| trust_error.lock().unwrap().take().unwrap_or_else(|| error(e)))?,
            _ = tokio::time::sleep(remaining), if !paused => return Err(error("SSH handshake timed out")),
        }
    };
    let material = tokio::select! {
        biased;
        _ = context.stop.cancelled() => return Err(TerminalError::cancelled()),
        material = options.auth_provider.prepare(context.owner(), context.stop.clone()) => material?,
    };
    if !matches!(
        (&material, target.summary.auth_kind),
        (SshAuthMaterial::Password(_), SshAuthKind::Password)
            | (SshAuthMaterial::PrivateKey(_), SshAuthKind::PrivateKey)
    ) {
        return Err(TerminalError::invalid(
            "authentication provider returned a different authentication kind",
        ));
    }
    let authenticate = async {
        let result = match material {
            SshAuthMaterial::Password(password) => session
                .authenticate_password(&options.username, password.expose())
                .await
                .map_err(error)?,
            SshAuthMaterial::PrivateKey(bytes) => {
                let text = std::str::from_utf8(bytes.expose())
                    .map_err(|_| TerminalError::invalid("private key must be unencrypted PEM"))?;
                let key = russh::keys::decode_secret_key(text, None)
                    .map_err(|_| TerminalError::invalid("invalid or encrypted private key"))?;
                let hash = session
                    .best_supported_rsa_hash()
                    .await
                    .map_err(error)?
                    .flatten();
                session
                    .authenticate_publickey(
                        &options.username,
                        PrivateKeyWithHashAlg::new(Arc::new(key), hash),
                    )
                    .await
                    .map_err(error)?
            }
        };
        if !result.success() {
            return Err(TerminalError::new(
                TerminalErrorKind::AuthenticationFailed,
                "SSH authentication rejected",
            ));
        }
        Ok(())
    };
    tokio::select! {
        biased;
        _ = context.stop.cancelled() => return Err(TerminalError::cancelled()),
        result = timeout(context.options.io.request_timeout, authenticate) => {
            result.map_err(|_| error("SSH authentication timed out"))??
        }
    }
    tokio::select! {
        biased;
        _ = context.stop.cancelled() => return Err(TerminalError::cancelled()),
        _ = options.auth_provider.authentication_succeeded(context.owner(), context.stop.clone()) => {}
    }
    Ok(Arc::new(Connection {
        client: session,
        socket,
        ended,
        sftp: OnceCell::new(),
    }))
}

pub(crate) async fn confirmation(
    channel: &mut Channel<client::Msg>,
    duration: Duration,
    output: Option<&super::OutputSink>,
) -> Result<()> {
    timeout(duration, async {
        loop {
            match channel.wait().await {
                Some(ChannelMsg::Success) => return Ok(()),
                Some(ChannelMsg::Failure) => return Err(error("SSH channel request rejected")),
                Some(ChannelMsg::Data { data } | ChannelMsg::ExtendedData { data, .. }) => {
                    if let Some(output) = output {
                        output.push(&data);
                    }
                }
                Some(ChannelMsg::Close) | None => return Err(lost()),
                _ => {}
            }
        }
    })
    .await
    .map_err(|_| error("SSH channel confirmation timed out"))?
}

pub(super) async fn run(
    target: SshStartTarget,
    command: Option<String>,
    mut context: Context,
) -> Result<()> {
    let connection = connect(&target, &context, None).await?;
    let result = async {
        let duration = context.options.io.request_timeout;
        let mut channel = request(duration, connection.client.channel_open_session()).await?;
        let default_size = portable_pty::PtySize::default();
        let size = context.size.lock().unwrap().unwrap_or(TerminalSize {
            rows: default_size.rows, cols: default_size.cols,
        });
        request(duration, channel.request_pty(
            true, "xterm-256color", size.cols as u32, size.rows as u32, 0, 0, &[],
        )).await?;
        confirmation(&mut channel, duration, context.output.as_deref()).await?;
        if let Some(command) = command {
            request(duration, channel.exec(true, command)).await?;
        } else {
            request(duration, channel.request_shell(true)).await?;
        }
        confirmation(&mut channel, duration, context.output.as_deref()).await?;
        context.event(DriverEvent::Started {
            id: context.id, connection: Some(connection.clone()),
        });
        let mut exit = None;
        loop {
            tokio::select! {
                biased;
                _ = context.force.cancelled() => {
                    context.event(DriverEvent::Forced(context.id));
                    connection.force_close();
                    break;
                }
                _ = context.stop.cancelled() => {
                    let _ = request(duration, channel.signal(russh::Sig::TERM)).await;
                    let _ = request(duration, channel.close()).await;
                    break;
                }
                _ = connection.ended.cancelled() => return Err(lost()),
                msg = channel.wait() => match msg {
                    Some(ChannelMsg::Data { data } | ChannelMsg::ExtendedData { data, .. }) => {
                        if let Some(output) = &context.output { output.push(&data); }
                    }
                    Some(ChannelMsg::ExitStatus { exit_status }) => exit = Some(exit_status),
                    Some(ChannelMsg::ExitSignal { signal_name, .. }) => return Err(TerminalError::new(
                        TerminalErrorKind::ProcessFailed, format!("remote process ended by {signal_name:?}"),
                    )),
                    Some(ChannelMsg::Close) | None => break,
                    _ => {},
                },
                input = context.input.recv() => match input {
                    Some(Input::Write(data, reply)) => {
                        let result = request(duration, channel.data(&data[..])).await;
                        let _ = reply.send(result);
                    }
                    Some(Input::Resize(size, reply)) => {
                        let result = request(duration, channel.window_change(size.cols as u32, size.rows as u32, 0, 0)).await;
                        let _ = reply.send(result);
                    }
                    None => { context.stop.cancel(); },
                }
            }
        }
        if context.stop.is_cancelled() || context.force.is_cancelled() {
            return Ok(());
        }
        match exit {
            Some(0) => Ok(()),
            Some(code) => Err(TerminalError::new(
                TerminalErrorKind::ProcessFailed, format!("remote process exited with status {code}"),
            )),
            None => Err(lost()),
        }
    }.await;
    context.event(DriverEvent::Cleaning(context.id));
    tokio::select! { _ = context.children_drained.cancelled() => {}, _ = context.force.cancelled() => {} }
    connection.close(&context).await;
    result
}
