use nooboard_ssh::{ConnectionState, DirectoryEntry, HostKey, Password, RemotePath, Transfer};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RemoteProfile {
    pub id: String,
    pub device_id: String,
    pub username: String,
    pub port: u16,
    /// Opaque vault reference, never a password. Not exported by desktop IPC.
    pub(crate) credential: Option<String>,
}
impl RemoteProfile {
    pub fn remembered(&self) -> bool {
        self.credential.is_some()
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Configuration {
    pub profiles: BTreeMap<String, RemoteProfile>,
    pub hosts: BTreeMap<String, HostKey>,
    pub download_directory: Option<PathBuf>,
}
impl Configuration {
    pub fn validate(&self) -> crate::Result<()> {
        if self.profiles.len() > 256
            || self.hosts.len() > 256
            || self
                .download_directory
                .as_ref()
                .is_some_and(|p| !p.is_absolute())
            || self.profiles.iter().any(|(id, p)| {
                id != &p.id
                    || p.port == 0
                    || p.username.is_empty()
                    || p.username.len() > 256
                    || p.username.chars().any(char::is_control)
            })
        {
            return Err(crate::Error::Configuration);
        }
        Ok(())
    }
    pub fn apply(&mut self, change: ConfigurationChange) {
        match change {
            ConfigurationChange::Profile(profile) => {
                self.profiles.insert(profile.id.clone(), profile);
            }
            ConfigurationChange::Host(id, key) => match key {
                Some(key) => {
                    self.hosts.insert(id, key);
                }
                None => {
                    self.hosts.remove(&id);
                }
            },
            ConfigurationChange::DownloadDirectory(path) => self.download_directory = Some(path),
        }
    }
}
pub(crate) enum ConfigurationChange {
    Profile(RemoteProfile),
    Host(String, Option<HostKey>),
    DownloadDirectory(PathBuf),
}
/// A fresh password or an explicit request to use an eligible saved credential.
/// Deliberately not serializable; this input never appears in a snapshot.
pub struct RemoteLogin {
    pub workspace: Option<String>,
    pub device_id: String,
    pub username: String,
    pub port: u16,
    pub password: Option<Password>,
    pub remember: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct RemoteChallenge {
    pub id: String,
    pub session_id: String,
    pub address: String,
    pub key: HostKey,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum DirectoryState {
    Loading,
    Partial,
    Complete,
    Stale,
    Error,
}
#[derive(Clone, Debug, Serialize)]
pub struct RemoteDirectory {
    pub path: RemotePath,
    pub entries: Vec<DirectoryEntry>,
    pub state: DirectoryState,
    pub error: Option<nooboard_ssh::Error>,
    pub(crate) generation: u64,
    pub(crate) cursor: Option<String>,
    pub(crate) query: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct RemoteWorkspace {
    pub id: String,
    pub profile_id: String,
    pub device_id: String,
    pub username: String,
    pub port: u16,
    pub address: Option<String>,
    pub terminal_id: String,
    pub files_id: String,
    pub terminal: ConnectionState,
    pub files: ConnectionState,
    pub initial_directory: Option<RemotePath>,
    pub root: Option<RemotePath>,
    pub generation: u64,
    #[serde(skip)]
    pub(crate) navigation: u64,
    pub directories: std::sync::Arc<Vec<RemoteDirectory>>,
    pub error: Option<String>,
    pub(crate) closed: bool,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct RemoteSnapshot {
    pub revision: u64,
    pub profiles: Vec<RemoteProfile>,
    pub workspaces: Vec<RemoteWorkspace>,
    pub challenges: Vec<RemoteChallenge>,
    pub transfers: Vec<Transfer>,
    pub download_directory: Option<PathBuf>,
}
pub(crate) fn host_id(device: &str, port: u16) -> String {
    format!("{device}:{port}")
}
pub(crate) fn profile_id(device: &str, username: &str, port: u16) -> String {
    // Length prefix prevents ambiguous usernames and device IDs.
    format!(
        "{}:{device}{port}:{}:{username}",
        device.len(),
        username.len()
    )
}

/// Immutable UI intent. A stale browser or a native picker cannot retarget a
/// request to a replacement file connection or a different browse root.
#[derive(Clone, Debug)]
pub struct RemoteFileTarget {
    pub workspace: String,
    pub files: String,
    pub generation: u64,
}
