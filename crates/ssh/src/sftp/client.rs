use crate::{
    connection::{channel::request_reply, client::Connection},
    *,
};
use russh::{ChannelMsg, Disconnect};
use russh_sftp::client::{Config, RawSftpSession};
use std::{sync::Mutex, time::Duration};
use tokio::time::timeout;

pub(crate) struct Client {
    pub raw: RawSftpSession,
    pub initial: RemotePath,
    connection: Mutex<Option<Connection>>,
    timeout: Duration,
}
impl Client {
    pub async fn open(connection: Connection, options: &Options) -> Result<Self> {
        let operation = async {
            let mut channel = connection.channel_open_session().await?;
            channel.request_subsystem(true, "sftp").await?;
            if !matches!(request_reply(&mut channel).await, Some(ChannelMsg::Success)) {
                return Err(Error::SftpUnavailable);
            }
            let raw = RawSftpSession::new_with_config(
                channel.into_stream(),
                Config {
                    request_timeout_secs: options.request_timeout.as_secs(),
                    max_packet_len: 256 * 1024,
                    ..Default::default()
                },
            );
            if raw.init().await?.version != 3 {
                return Err(Error::SftpUnavailable);
            }
            let paths = raw.realpath(".").await?;
            if paths.files.len() != 1 {
                return Err(Error::InvalidPath);
            }
            let initial = RemotePath::new(paths.files[0].filename.clone())?;
            if raw
                .stat(initial.as_str())
                .await?
                .attrs
                .permissions
                .map(|p| p & 0o170000)
                != Some(0o040000)
            {
                return Err(Error::NotDirectory);
            }
            Ok::<_, Error>((raw, initial))
        };
        match timeout(options.request_timeout, operation)
            .await
            .unwrap_or(Err(Error::Timeout))
        {
            Ok((raw, initial)) => Ok(Self {
                raw,
                initial,
                connection: Mutex::new(Some(connection)),
                timeout: options.request_timeout,
            }),
            Err(e) => {
                let _ = timeout(
                    options.request_timeout,
                    connection.disconnect(Disconnect::ByApplication, "file setup failed", "en"),
                )
                .await;
                let _ = timeout(options.request_timeout, connection).await;
                Err(e)
            }
        }
    }
    pub fn is_closed(&self) -> bool {
        self.connection
            .lock()
            .map_or(true, |c| c.as_ref().is_none_or(Connection::is_closed))
    }
    pub async fn shutdown(&self) {
        let _ = self.raw.close_session();
        let connection = self
            .connection
            .lock()
            .expect("file connection poisoned")
            .take();
        if let Some(connection) = connection {
            let _ = timeout(
                self.timeout,
                connection.disconnect(Disconnect::ByApplication, "file service closed", "en"),
            )
            .await;
            let _ = timeout(self.timeout, connection).await;
        }
    }
}
pub(crate) fn eof(error: &russh_sftp::client::error::Error) -> bool {
    matches!(error, russh_sftp::client::error::Error::Status(s) if s.status_code == russh_sftp::protocol::StatusCode::Eof)
}
