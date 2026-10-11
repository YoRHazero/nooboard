use nooboard_ssh::*;
use russh::{
    Channel, ChannelId,
    server::{self, Auth, Session},
};
use russh_sftp::protocol::*;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{net::TcpListener, task::JoinSet};

#[derive(Default)]
pub struct Data {
    pub reads: AtomicUsize,
    pub closes: AtomicUsize,
    pub passwords: AtomicUsize,
    pub transports: AtomicUsize,
    pub files: Mutex<HashMap<String, Vec<u8>>>,
    pub terminals: Mutex<Vec<server::Handle>>,
    pub file_connections: Mutex<Vec<server::Handle>>,
    pub slow_writes: bool,
    pub drop_rename_reply: bool,
    pub deny_sftp: bool,
}
pub struct Server {
    pub address: SocketAddr,
    pub key: HostKey,
    pub data: Arc<Data>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Server {
    pub async fn start(data: Data) -> Self {
        let key =
            russh::keys::PrivateKey::random(&mut rand::rng(), russh::keys::Algorithm::Ed25519)
                .unwrap();
        let trusted = HostKey {
            algorithm: key.algorithm().to_string(),
            public_key: key.public_key().to_openssh().unwrap(),
            fingerprint: key.fingerprint(russh::keys::HashAlg::Sha256).to_string(),
        };
        let config = Arc::new(server::Config {
            keys: vec![key],
            auth_rejection_time: Duration::ZERO,
            auth_rejection_time_initial: Some(Duration::ZERO),
            ..Default::default()
        });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let data = Arc::new(data);
        let state = data.clone();
        let task = tokio::spawn(async move {
            let mut jobs = JoinSet::new();
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        let (stream, _) = result.unwrap();
                        state.transports.fetch_add(1, Ordering::SeqCst);
                        let handler = Handler { data: state.clone(), channels: HashMap::new() };
                        let config = config.clone();
                        jobs.spawn(async move { if let Ok(session) = server::run_stream(config, stream, handler).await { let _ = session.await; } });
                    },
                    _ = jobs.join_next(), if !jobs.is_empty() => {},
                }
            }
        });
        Self {
            address,
            key: trusted,
            data,
            task,
        }
    }
    pub fn options(&self) -> Connect {
        Connect {
            device_id: "paired-device".into(),
            addresses: vec![self.address],
            username: "tester".into(),
            password: Password::new("test-password".into()),
            trusted_key: Some(self.key.clone()),
            size: PtySize::default(),
            scope: ConnectScope::Both,
        }
    }
    pub async fn connect(&self, ssh: &Ssh) -> String {
        let id = ssh.connect(self.options()).await.unwrap();
        wait(ssh, |s| {
            s.sessions.iter().any(|s| {
                s.id == id
                    && s.terminal == ConnectionState::Ready
                    && s.files == ConnectionState::Ready
            })
        })
        .await;
        id
    }
}
pub async fn wait(ssh: &Ssh, predicate: impl Fn(&Snapshot) -> bool) {
    let mut state = ssh.subscribe();
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            if predicate(&state.borrow_and_update()) {
                break;
            }
            state.changed().await.unwrap();
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out: {:?}", ssh.snapshot()));
}
struct Handler {
    data: Arc<Data>,
    channels: HashMap<ChannelId, Channel<server::Msg>>,
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
    async fn channel_open_session(
        &mut self,
        channel: Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        _: &mut Session,
    ) -> std::result::Result<(), Self::Error> {
        self.channels.insert(channel.id(), channel);
        reply.accept().await;
        Ok(())
    }
    async fn pty_request(
        &mut self,
        channel: ChannelId,
        _: &str,
        _: u32,
        _: u32,
        _: u32,
        _: u32,
        _: &[(russh::Pty, u32)],
        session: &mut Session,
    ) -> std::result::Result<(), Self::Error> {
        session.channel_success(channel)?;
        Ok(())
    }
    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> std::result::Result<(), Self::Error> {
        self.data.terminals.lock().unwrap().push(session.handle());
        session.channel_success(channel)?;
        session.data(channel, b"ready\r\n".to_vec())?;
        Ok(())
    }
    async fn data(
        &mut self,
        channel: ChannelId,
        bytes: &[u8],
        session: &mut Session,
    ) -> std::result::Result<(), Self::Error> {
        if self.channels.contains_key(&channel) {
            session.data(channel, bytes.to_vec())?;
        }
        Ok(())
    }
    async fn subsystem_request(
        &mut self,
        channel: ChannelId,
        name: &str,
        session: &mut Session,
    ) -> std::result::Result<(), Self::Error> {
        if name == "sftp" && !self.data.deny_sftp {
            let handle = session.handle();
            self.data
                .file_connections
                .lock()
                .unwrap()
                .push(handle.clone());
            session.channel_success(channel)?;
            let stream = self.channels.remove(&channel).unwrap().into_stream();
            russh_sftp::server::run(
                stream,
                Files {
                    data: self.data.clone(),
                    directories: HashMap::new(),
                    open: HashMap::new(),
                    ssh: handle,
                },
            )
            .await;
        } else {
            session.channel_failure(channel)?;
        }
        Ok(())
    }
}
struct Files {
    data: Arc<Data>,
    directories: HashMap<String, usize>,
    open: HashMap<String, String>,
    ssh: server::Handle,
}
fn attrs(directory: bool, size: usize) -> FileAttributes {
    FileAttributes {
        permissions: Some(if directory { 0o040755 } else { 0o100644 }),
        size: Some(size as u64),
        mtime: Some(100),
        ..Default::default()
    }
}
fn ok(id: u32) -> Status {
    Status {
        id,
        status_code: StatusCode::Ok,
        error_message: String::new(),
        language_tag: String::new(),
    }
}
type S<T> = std::result::Result<T, StatusCode>;
impl Files {
    fn attributes(&self, path: &str) -> S<FileAttributes> {
        if path == "/denied" {
            return Err(StatusCode::PermissionDenied);
        }
        if path == "/root" || path == "/root/sub" {
            return Ok(attrs(true, 0));
        }
        self.data
            .files
            .lock()
            .unwrap()
            .get(path)
            .map(|b| attrs(false, b.len()))
            .ok_or(StatusCode::NoSuchFile)
    }
}
impl russh_sftp::server::Handler for Files {
    type Error = StatusCode;
    fn unimplemented(&self) -> StatusCode {
        StatusCode::OpUnsupported
    }
    async fn realpath(&mut self, id: u32, path: String) -> S<Name> {
        let path = match path.as_str() {
            "." | "/root/." => "/root".into(),
            "/root/sub/.." => "/root".into(),
            _ => path,
        };
        Ok(Name {
            id,
            files: vec![File::dummy(path)],
        })
    }
    async fn stat(&mut self, id: u32, path: String) -> S<Attrs> {
        Ok(Attrs {
            id,
            attrs: self.attributes(&path)?,
        })
    }
    async fn lstat(&mut self, id: u32, path: String) -> S<Attrs> {
        Ok(Attrs {
            id,
            attrs: self.attributes(&path)?,
        })
    }
    async fn opendir(&mut self, id: u32, path: String) -> S<Handle> {
        if path != "/root" && path != "/root/sub" {
            return Err(StatusCode::NoSuchFile);
        }
        let handle = format!("directory-{id}");
        self.directories.insert(handle.clone(), 0);
        Ok(Handle { id, handle })
    }
    async fn readdir(&mut self, id: u32, handle: String) -> S<Name> {
        self.data.reads.fetch_add(1, Ordering::SeqCst);
        let position = self
            .directories
            .get_mut(&handle)
            .ok_or(StatusCode::Failure)?;
        if *position >= 450 {
            return Err(StatusCode::Eof);
        }
        let files = (*position..(*position + 150).min(450))
            .map(|n| File::new(format!("file-{n}"), attrs(false, 1)))
            .collect();
        *position += 150;
        Ok(Name { id, files })
    }
    async fn close(&mut self, id: u32, handle: String) -> S<Status> {
        self.data.closes.fetch_add(1, Ordering::SeqCst);
        self.directories.remove(&handle);
        self.open.remove(&handle);
        Ok(ok(id))
    }
    async fn open(
        &mut self,
        id: u32,
        filename: String,
        flags: OpenFlags,
        _: FileAttributes,
    ) -> S<Handle> {
        let mut files = self.data.files.lock().unwrap();
        if flags.contains(OpenFlags::CREATE) {
            if flags.contains(OpenFlags::EXCLUDE) && files.contains_key(&filename) {
                return Err(StatusCode::Failure);
            }
            files.entry(filename.clone()).or_default();
        } else if !files.contains_key(&filename) {
            return Err(StatusCode::NoSuchFile);
        }
        let handle = format!("file-{id}");
        self.open.insert(handle.clone(), filename);
        Ok(Handle { id, handle })
    }
    async fn fstat(&mut self, id: u32, handle: String) -> S<Attrs> {
        Ok(Attrs {
            id,
            attrs: self.attributes(self.open.get(&handle).ok_or(StatusCode::Failure)?)?,
        })
    }
    async fn read(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        len: u32,
    ) -> S<russh_sftp::protocol::Data> {
        let files = self.data.files.lock().unwrap();
        let bytes = files
            .get(self.open.get(&handle).ok_or(StatusCode::Failure)?)
            .ok_or(StatusCode::NoSuchFile)?;
        if offset as usize >= bytes.len() {
            return Err(StatusCode::Eof);
        }
        Ok(russh_sftp::protocol::Data {
            id,
            data: bytes[offset as usize..(offset as usize + len as usize).min(bytes.len())]
                .to_vec(),
        })
    }
    async fn write(&mut self, id: u32, handle: String, offset: u64, bytes: Vec<u8>) -> S<Status> {
        if self.data.slow_writes {
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
        let mut files = self.data.files.lock().unwrap();
        let file = files
            .get_mut(self.open.get(&handle).ok_or(StatusCode::Failure)?)
            .ok_or(StatusCode::NoSuchFile)?;
        file.resize(file.len().max(offset as usize + bytes.len()), 0);
        file[offset as usize..offset as usize + bytes.len()].copy_from_slice(&bytes);
        Ok(ok(id))
    }
    async fn rename(&mut self, id: u32, from: String, to: String) -> S<Status> {
        {
            let mut files = self.data.files.lock().unwrap();
            if files.contains_key(&to) {
                return Err(StatusCode::Failure);
            }
            let value = files.remove(&from).ok_or(StatusCode::NoSuchFile)?;
            files.insert(to, value);
        }
        if self.data.drop_rename_reply {
            let _ = self
                .ssh
                .disconnect(
                    russh::Disconnect::ByApplication,
                    "test disconnect".into(),
                    "en".into(),
                )
                .await;
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        Ok(ok(id))
    }
    async fn remove(&mut self, id: u32, filename: String) -> S<Status> {
        self.data
            .files
            .lock()
            .unwrap()
            .remove(&filename)
            .ok_or(StatusCode::NoSuchFile)?;
        Ok(ok(id))
    }
}
