use crate::{Password, PtySize};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostKey {
    pub algorithm: String,
    pub fingerprint: String,
    pub public_key: String,
}

#[derive(Debug)]
pub struct Connect {
    /// Stable identity supplied by the application's paired-device registry.
    pub device_id: String,
    pub addresses: Vec<SocketAddr>,
    pub username: String,
    pub password: Password,
    pub trusted_key: Option<HostKey>,
    pub size: PtySize,
    /// Core can replace only a failed terminal or file connection and keep the
    /// healthy half of its logical workspace, using the new returned session ID.
    pub scope: ConnectScope,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ConnectScope {
    #[default]
    Both,
    Terminal,
    Files,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum ConnectionState {
    Connecting,
    Ready,
    Ended,
    Failed(crate::Error),
    Closed,
}

#[derive(Clone, Debug, Serialize)]
pub struct SessionStatus {
    pub id: String,
    pub device_id: String,
    pub username: String,
    pub address: Option<SocketAddr>,
    pub terminal: ConnectionState,
    pub files: ConnectionState,
    pub initial_directory: Option<crate::RemotePath>,
    pub attempt_finished: bool,
    /// Sticky authentication outcome, independent of a fast shell exit.
    pub authenticated: bool,
}
