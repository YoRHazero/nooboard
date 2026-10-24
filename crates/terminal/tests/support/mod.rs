use nooboard_terminal::*;
use russh::{
    Channel, ChannelId,
    server::{self, Auth, Session},
};
use russh_sftp::protocol::{self, *};
use std::{
    collections::HashMap,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    net::{TcpListener, TcpStream},
    task::JoinSet,
};
#[derive(Default)]
pub struct Settings {
    pub deny_sftp: bool,
    pub slow_writes: bool,
    pub atomic_replace: bool,
    pub drop_commit_reply: bool,
}
pub struct Data {
    pub passwords: AtomicUsize,
    pub transports: AtomicUsize,
    pub subsystems: AtomicUsize,
    pub connections: Mutex<Vec<server::Handle>>,
    pub root: PathBuf,
    pub settings: Settings,
}
pub struct Server {
    pub address: SocketAddr,
    pub key: SshPublicKey,
    pub data: Arc<Data>,
    pub directory: tempfile::TempDir,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Server {
    pub async fn start(settings: Settings) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let key =
            russh::keys::PrivateKey::random(&mut rand::rng(), russh::keys::Algorithm::Ed25519)
                .unwrap();
        let public = SshPublicKey {
            algorithm: key.algorithm().to_string(),
            bytes: key.public_key().to_bytes().unwrap(),
        };
        let config = Arc::new(server::Config {
            keys: vec![key],
            auth_rejection_time: Duration::ZERO,
            auth_rejection_time_initial: Some(Duration::ZERO),
            ..Default::default()
        });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let data = Arc::new(Data {
            passwords: AtomicUsize::new(0),
            transports: AtomicUsize::new(0),
            subsystems: AtomicUsize::new(0),
            connections: Mutex::new(vec![]),
            root: directory.path().to_owned(),
            settings,
        });
        let state = data.clone();
        let task = tokio::spawn(async move {
            let mut jobs = JoinSet::new();
            loop {
                tokio::select! {
                    accepted=listener.accept()=>{let(stream,_)=accepted.unwrap();state.transports.fetch_add(1,Ordering::SeqCst);let handler=Handler{data:state.clone(),channels:HashMap::new(),forwards:HashMap::new(),jobs:JoinSet::new()};let config=config.clone();jobs.spawn(async move{if let Ok(session)=server::run_stream(config,stream,handler).await{let _=session.await;}});},
                    _=jobs.join_next(),if !jobs.is_empty()=>{}
                }
            }
        });
        Self {
            address,
            key: public,
            data,
            directory,
            task,
        }
    }
    pub fn target(&self, auth: Arc<AuthProvider>) -> SshStartTarget {
        SshStartTarget {
            config_id: SshTargetConfigId::new(),
            config_name: "loopback".into(),
            summary: SshTargetSummary {
                address: SshAddressSource::Host(self.address.ip().to_string()),
                port: self.address.port(),
                username: "tester".into(),
                auth_kind: SshAuthKind::Password,
            },
            provider: Arc::new(Provider {
                address: self.address,
                auth,
            }),
        }
    }
    pub async fn disconnect(&self) {
        let handles = self.data.connections.lock().unwrap().clone();
        for handle in handles {
            let _ = handle
                .disconnect(
                    russh::Disconnect::ByApplication,
                    "test disconnect".into(),
                    "".into(),
                )
                .await;
        }
    }
}
pub struct Trust {
    pub expected: SshPublicKey,
    pub checked: Arc<AtomicUsize>,
    pub reject: bool,
}
impl SshHostVerifier for Trust {
    fn verify(
        &self,
        request: SshHostVerificationRequest,
        _: CancellationToken,
    ) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move {
            self.checked.fetch_add(1, Ordering::SeqCst);
            if !self.reject && request.server_key == self.expected {
                Ok(())
            } else {
                Err(TerminalError::new(
                    TerminalErrorKind::HostKeyRejected,
                    "test trust rejected",
                ))
            }
        })
    }
}
pub struct AuthProvider {
    pub prepared: AtomicUsize,
    pub succeeded: AtomicUsize,
    pub trust_checked: Arc<AtomicUsize>,
    pub password: String,
}
impl AuthProvider {
    pub fn new(checked: Arc<AtomicUsize>) -> Arc<Self> {
        Arc::new(Self {
            prepared: AtomicUsize::new(0),
            succeeded: AtomicUsize::new(0),
            trust_checked: checked,
            password: "test-password".into(),
        })
    }
}
impl SshAuthProvider for AuthProvider {
    fn prepare(
        &self,
        _: SshConnectionOwnerRunId,
        _: CancellationToken,
    ) -> BoxFuture<'_, Result<SshAuthMaterial>> {
        Box::pin(async move {
            assert!(self.trust_checked.load(Ordering::SeqCst) > 0);
            self.prepared.fetch_add(1, Ordering::SeqCst);
            Ok(SshAuthMaterial::Password(SecretString::new(
                self.password.clone(),
            )))
        })
    }
    fn authentication_succeeded(
        &self,
        _: SshConnectionOwnerRunId,
        _: CancellationToken,
    ) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            self.succeeded.fetch_add(1, Ordering::SeqCst);
        })
    }
}
struct Provider {
    address: SocketAddr,
    auth: Arc<AuthProvider>,
}
impl SshConnectionProvider for Provider {
    fn prepare(&self) -> BoxFuture<'_, Result<SshConnectOptions>> {
        Box::pin(async move {
            Ok(SshConnectOptions {
                address: self.address,
                username: "tester".into(),
                auth_provider: self.auth.clone(),
            })
        })
    }
}
pub fn service(server: &Server, reject: bool) -> (TerminalService, Terminal, Arc<AuthProvider>) {
    let checked = Arc::new(AtomicUsize::new(0));
    let auth = AuthProvider::new(checked.clone());
    let mut options = TerminalServiceOptions::default();
    options.stop.stop_grace_timeout = Duration::from_millis(200);
    options.stop.force_stop_timeout = Duration::from_secs(2);
    options.stop.shutdown_timeout = Duration::from_secs(5);
    options.io.request_timeout = Duration::from_secs(1);
    let service = TerminalService::start(
        options,
        TerminalServiceDependencies {
            host_verifier: Arc::new(Trust {
                expected: server.key.clone(),
                checked,
                reject,
            }),
        },
    )
    .unwrap();
    let terminal = service.handle();
    (service, terminal, auth)
}
pub async fn wait(terminal: &Terminal, predicate: impl Fn(&TerminalSnapshot) -> bool) {
    let mut sub = terminal.subscribe();
    tokio::time::timeout(Duration::from_secs(8), async {
        if predicate(&sub.initial_snapshot) {
            return;
        }
        while let Some(snapshot) = sub.snapshots.next().await {
            if predicate(&snapshot) {
                return;
            }
        }
        panic!("service closed");
    })
    .await
    .unwrap_or_else(|_| panic!("state timeout: {:?}", terminal.snapshot()));
}
pub async fn terminal(terminal: &Terminal, target: SshStartTarget) -> TerminalRunId {
    let id = terminal
        .start_terminal(StartTerminalRequest {
            config_id: TerminalConfigId::new(),
            config_name: "remote terminal".into(),
            target: TerminalStartTarget::Ssh(target),
            dependencies: vec![],
        })
        .await
        .unwrap();
    wait(terminal, |s| {
        s.terminal_runs
            .iter()
            .any(|r| r.id == id && matches!(r.state, TerminalRunState::Running))
    })
    .await;
    id
}
struct Handler {
    data: Arc<Data>,
    channels: HashMap<ChannelId, Channel<server::Msg>>,
    forwards: HashMap<u32, CancellationToken>,
    jobs: JoinSet<()>,
}
impl Drop for Handler {
    fn drop(&mut self) {
        for stop in self.forwards.values() {
            stop.cancel();
        }
    }
}
impl server::Handler for Handler {
    type Error = russh::Error;
    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> std::result::Result<Auth, Self::Error> {
        self.data.passwords.fetch_add(1, Ordering::SeqCst);
        Ok(if user == "tester" && password == "test-password" {
            Auth::Accept
        } else {
            Auth::reject()
        })
    }
    async fn auth_publickey(
        &mut self,
        user: &str,
        _: &russh::keys::PublicKey,
    ) -> std::result::Result<Auth, Self::Error> {
        Ok(if user == "tester" {
            Auth::Accept
        } else {
            Auth::reject()
        })
    }
    async fn channel_open_session(
        &mut self,
        channel: Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        session: &mut Session,
    ) -> std::result::Result<(), Self::Error> {
        self.data.connections.lock().unwrap().push(session.handle());
        self.channels.insert(channel.id(), channel);
        reply.accept().await;
        Ok(())
    }
    async fn pty_request(
        &mut self,
        id: ChannelId,
        _: &str,
        _: u32,
        _: u32,
        _: u32,
        _: u32,
        _: &[(russh::Pty, u32)],
        session: &mut Session,
    ) -> std::result::Result<(), Self::Error> {
        session.channel_success(id)?;
        Ok(())
    }
    async fn shell_request(
        &mut self,
        id: ChannelId,
        session: &mut Session,
    ) -> std::result::Result<(), Self::Error> {
        session.channel_success(id)?;
        session.data(id, b"test shell\r\n".to_vec())?;
        Ok(())
    }
    async fn exec_request(
        &mut self,
        id: ChannelId,
        command: &[u8],
        session: &mut Session,
    ) -> std::result::Result<(), Self::Error> {
        session.channel_success(id)?;
        if command == b"finish" {
            session.data(id, b"done\r\n".to_vec())?;
            session.exit_status_request(id, 0)?;
            session.eof(id)?;
            session.close(id)?;
        } else {
            session.data(id, b"::nooboard:v1:ready::\r\n".to_vec())?;
        }
        Ok(())
    }
    async fn data(
        &mut self,
        id: ChannelId,
        bytes: &[u8],
        session: &mut Session,
    ) -> std::result::Result<(), Self::Error> {
        if self.channels.contains_key(&id) {
            session.data(id, bytes.to_vec())?;
        }
        Ok(())
    }
    async fn channel_close(
        &mut self,
        id: ChannelId,
        session: &mut Session,
    ) -> std::result::Result<(), Self::Error> {
        self.channels.remove(&id);
        session.close(id)?;
        Ok(())
    }
    async fn subsystem_request(
        &mut self,
        id: ChannelId,
        name: &str,
        session: &mut Session,
    ) -> std::result::Result<(), Self::Error> {
        if name != "sftp" || self.data.settings.deny_sftp {
            session.channel_failure(id)?;
            return Ok(());
        }
        self.data.subsystems.fetch_add(1, Ordering::SeqCst);
        session.channel_success(id)?;
        let stream = self.channels.remove(&id).unwrap().into_stream();
        russh_sftp::server::run(
            stream,
            Files {
                data: self.data.clone(),
                open: HashMap::new(),
                directories: HashMap::new(),
                ssh: session.handle(),
            },
        )
        .await;
        Ok(())
    }
    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<server::Msg>,
        host: &str,
        port: u32,
        _: &str,
        _: u32,
        reply: server::ChannelOpenHandle,
        _: &mut Session,
    ) -> std::result::Result<(), Self::Error> {
        let target = TcpStream::connect((host, port as u16)).await;
        match target {
            Ok(mut target) => {
                reply.accept().await;
                self.jobs.spawn(async move {
                    let mut channel = channel.into_stream();
                    let _ = tokio::io::copy_bidirectional(&mut channel, &mut target).await;
                });
            }
            Err(_) => {
                reply.reject(russh::ChannelOpenFailure::ConnectFailed).await;
            }
        }
        Ok(())
    }
    async fn tcpip_forward(
        &mut self,
        address: &str,
        port: &mut u32,
        session: &mut Session,
    ) -> std::result::Result<bool, Self::Error> {
        let listener = match TcpListener::bind((address, *port as u16)).await {
            Ok(l) => l,
            Err(_) => return Ok(false),
        };
        *port = listener.local_addr().unwrap().port() as u32;
        let listen_port = *port;
        let address = address.to_owned();
        let handle = session.handle();
        let stop = CancellationToken::new();
        self.forwards.insert(listen_port, stop.clone());
        self.jobs.spawn(async move{let mut flows=JoinSet::new();loop{tokio::select!{
            _=stop.cancelled()=>break,
            accepted=listener.accept()=>{let(mut stream,origin)=accepted.unwrap();let handle=handle.clone();let address=address.clone();flows.spawn(async move{if let Ok(channel)=handle.channel_open_forwarded_tcpip(address,listen_port,origin.ip().to_string(),origin.port() as u32).await{let mut channel=channel.into_stream();let _=tokio::io::copy_bidirectional(&mut stream,&mut channel).await;}});},
            _=flows.join_next(),if !flows.is_empty()=>{}
        }}});
        Ok(true)
    }
    async fn cancel_tcpip_forward(
        &mut self,
        _: &str,
        port: u32,
        _: &mut Session,
    ) -> std::result::Result<bool, Self::Error> {
        if let Some(stop) = self.forwards.remove(&port) {
            stop.cancel();
        }
        Ok(true)
    }
}
struct Files {
    data: Arc<Data>,
    open: HashMap<String, std::fs::File>,
    directories: HashMap<String, Vec<protocol::File>>,
    ssh: server::Handle,
}
type S<T> = std::result::Result<T, StatusCode>;
fn status(id: u32) -> Status {
    Status {
        id,
        status_code: StatusCode::Ok,
        error_message: String::new(),
        language_tag: String::new(),
    }
}
fn fs_error(e: std::io::Error) -> StatusCode {
    match e.kind() {
        std::io::ErrorKind::NotFound => StatusCode::NoSuchFile,
        std::io::ErrorKind::PermissionDenied => StatusCode::PermissionDenied,
        _ => StatusCode::Failure,
    }
}
fn attrs(metadata: std::fs::Metadata) -> FileAttributes {
    FileAttributes {
        size: Some(metadata.len()),
        permissions: Some(if metadata.is_symlink() {
            0o120777
        } else if metadata.is_dir() {
            0o040755
        } else {
            0o100644
        }),
        ..Default::default()
    }
}
impl Files {
    fn path(&self, path: &str) -> S<PathBuf> {
        if path == "/denied" {
            return Err(StatusCode::PermissionDenied);
        }
        let relative = Path::new(path.trim_start_matches('/'));
        if relative.components().any(|c| {
            !matches!(
                c,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        }) {
            return Err(StatusCode::PermissionDenied);
        }
        Ok(self.data.root.join(relative))
    }
    async fn committed(&self) {
        if self.data.settings.drop_commit_reply {
            let _ = self
                .ssh
                .disconnect(
                    russh::Disconnect::ByApplication,
                    "drop commit acknowledgement".into(),
                    "".into(),
                )
                .await;
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }
}
impl russh_sftp::server::Handler for Files {
    type Error = StatusCode;
    fn unimplemented(&self) -> StatusCode {
        StatusCode::OpUnsupported
    }
    async fn init(&mut self, _: u32, _: HashMap<String, String>) -> S<Version> {
        let mut v = Version::new();
        if self.data.settings.atomic_replace {
            v.extensions
                .insert("posix-rename@openssh.com".into(), "1".into());
        }
        Ok(v)
    }
    async fn realpath(&mut self, id: u32, path: String) -> S<Name> {
        Ok(Name {
            id,
            files: vec![protocol::File::dummy(if path == "." {
                "/".to_owned()
            } else {
                path
            })],
        })
    }
    async fn stat(&mut self, id: u32, path: String) -> S<Attrs> {
        Ok(Attrs {
            id,
            attrs: attrs(std::fs::metadata(self.path(&path)?).map_err(fs_error)?),
        })
    }
    async fn lstat(&mut self, id: u32, path: String) -> S<Attrs> {
        Ok(Attrs {
            id,
            attrs: attrs(std::fs::symlink_metadata(self.path(&path)?).map_err(fs_error)?),
        })
    }
    async fn opendir(&mut self, id: u32, path: String) -> S<Handle> {
        let entries = std::fs::read_dir(self.path(&path)?)
            .map_err(fs_error)?
            .map(|e| {
                let e = e.map_err(fs_error)?;
                Ok(protocol::File::new(
                    e.file_name().to_string_lossy().into_owned(),
                    attrs(std::fs::symlink_metadata(e.path()).map_err(fs_error)?),
                ))
            })
            .collect::<S<Vec<_>>>()?;
        let handle = format!("dir-{id}");
        self.directories.insert(handle.clone(), entries);
        Ok(Handle { id, handle })
    }
    async fn readdir(&mut self, id: u32, handle: String) -> S<Name> {
        let entries = self
            .directories
            .get_mut(&handle)
            .ok_or(StatusCode::Failure)?;
        if entries.is_empty() {
            return Err(StatusCode::Eof);
        }
        Ok(Name {
            id,
            files: std::mem::take(entries),
        })
    }
    async fn open(
        &mut self,
        id: u32,
        path: String,
        flags: OpenFlags,
        _: FileAttributes,
    ) -> S<Handle> {
        let mut options = std::fs::OpenOptions::new();
        options
            .read(flags.contains(OpenFlags::READ))
            .write(flags.contains(OpenFlags::WRITE))
            .create(flags.contains(OpenFlags::CREATE))
            .create_new(flags.contains(OpenFlags::EXCLUDE))
            .truncate(flags.contains(OpenFlags::TRUNCATE));
        let file = options.open(self.path(&path)?).map_err(fs_error)?;
        let handle = format!("file-{id}");
        self.open.insert(handle.clone(), file);
        Ok(Handle { id, handle })
    }
    async fn close(&mut self, id: u32, handle: String) -> S<Status> {
        self.open.remove(&handle);
        self.directories.remove(&handle);
        Ok(status(id))
    }
    async fn read(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        length: u32,
    ) -> S<protocol::Data> {
        use std::io::{Read, Seek};
        let file = self.open.get_mut(&handle).ok_or(StatusCode::Failure)?;
        file.seek(std::io::SeekFrom::Start(offset))
            .map_err(fs_error)?;
        let mut data = vec![0; length as usize];
        let n = file.read(&mut data).map_err(fs_error)?;
        if n == 0 {
            return Err(StatusCode::Eof);
        }
        data.truncate(n);
        Ok(protocol::Data { id, data })
    }
    async fn write(&mut self, id: u32, handle: String, offset: u64, data: Vec<u8>) -> S<Status> {
        use std::io::{Seek, Write};
        if self.data.settings.slow_writes {
            tokio::time::sleep(Duration::from_millis(40)).await;
        }
        let file = self.open.get_mut(&handle).ok_or(StatusCode::Failure)?;
        file.seek(std::io::SeekFrom::Start(offset))
            .map_err(fs_error)?;
        file.write_all(&data).map_err(fs_error)?;
        Ok(status(id))
    }
    async fn mkdir(&mut self, id: u32, path: String, _: FileAttributes) -> S<Status> {
        std::fs::create_dir(self.path(&path)?).map_err(fs_error)?;
        Ok(status(id))
    }
    async fn rmdir(&mut self, id: u32, path: String) -> S<Status> {
        std::fs::remove_dir(self.path(&path)?).map_err(fs_error)?;
        Ok(status(id))
    }
    async fn remove(&mut self, id: u32, path: String) -> S<Status> {
        std::fs::remove_file(self.path(&path)?).map_err(fs_error)?;
        Ok(status(id))
    }
    async fn rename(&mut self, id: u32, from: String, to: String) -> S<Status> {
        let from = self.path(&from)?;
        let to = self.path(&to)?;
        if std::fs::symlink_metadata(&to).is_ok() {
            return Err(StatusCode::Failure);
        }
        std::fs::rename(from, to).map_err(fs_error)?;
        self.committed().await;
        Ok(status(id))
    }
    async fn extended(&mut self, id: u32, name: String, bytes: Vec<u8>) -> S<Packet> {
        if name != "posix-rename@openssh.com" || !self.data.settings.atomic_replace {
            return Err(StatusCode::OpUnsupported);
        }
        fn string(bytes: &mut &[u8]) -> S<String> {
            if bytes.len() < 4 {
                return Err(StatusCode::BadMessage);
            }
            let len = u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize;
            *bytes = &bytes[4..];
            if bytes.len() < len {
                return Err(StatusCode::BadMessage);
            }
            let value =
                String::from_utf8(bytes[..len].to_vec()).map_err(|_| StatusCode::BadMessage)?;
            *bytes = &bytes[len..];
            Ok(value)
        }
        let mut bytes = bytes.as_slice();
        let from = string(&mut bytes)?;
        let to = string(&mut bytes)?;
        std::fs::rename(self.path(&from)?, self.path(&to)?).map_err(fs_error)?;
        self.committed().await;
        Ok(Packet::Status(status(id)))
    }
}
