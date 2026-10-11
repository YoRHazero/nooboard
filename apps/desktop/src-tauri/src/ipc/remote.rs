//! Typed SSH workspace protocol. Passwords exist only in the login request.
use super::errors::{self, UiError};
use nooboard_core as core;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, JsonSchema)]
#[serde(
    tag = "type",
    content = "data",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub enum RemoteRequest {
    Connect {
        workspace: Option<String>,
        device: String,
        username: String,
        port: u16,
        password: Option<String>,
        remember: bool,
    },
    HostKey {
        id: String,
        accept: bool,
    },
    ForgetHostKey {
        device: String,
        port: u16,
    },
    Close {
        workspace: String,
    },
    Input {
        workspace: String,
        session: String,
        bytes: Vec<u8>,
    },
    Resize {
        workspace: String,
        session: String,
        columns: u32,
        rows: u32,
    },
    Replay {
        workspace: String,
        session: String,
        after: String,
    },
    Navigate {
        target: RemoteFileTarget,
        path: String,
    },
    List {
        target: RemoteFileTarget,
        path: String,
        more: bool,
    },
    Collapse {
        target: RemoteFileTarget,
        path: String,
    },
    SelectUpload {
        target: RemoteFileTarget,
        directory: String,
    },
    Upload {
        target: RemoteFileTarget,
        directory: String,
        paths: Vec<String>,
    },
    Download {
        target: RemoteFileTarget,
        paths: Vec<String>,
    },
    SelectDownloadDirectory,
    Cancel {
        id: String,
    },
    Retry {
        id: String,
    },
    ForgetTransfer {
        id: String,
    },
    Reveal {
        id: String,
    },
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoteFileTarget {
    pub workspace: String,
    pub files: String,
    pub generation: String,
}
impl RemoteFileTarget {
    pub fn into_core(self) -> Result<core::RemoteFileTarget, UiError> {
        Ok(core::RemoteFileTarget {
            workspace: self.workspace,
            files: self.files,
            generation: self
                .generation
                .parse()
                .map_err(|_| errors::ui("configuration"))?,
        })
    }
}
#[derive(Serialize, JsonSchema)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum RemoteReply {
    Done,
    Connected { id: String },
    Replay(RemoteReplay),
}
#[derive(Clone, Serialize, JsonSchema)]
pub struct RemoteReplay {
    pub start: String,
    pub end: String,
    pub gap: bool,
    pub bytes: Vec<u8>,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TerminalFrame {
    pub session_id: String,
    pub offset: String,
    pub bytes: Vec<u8>,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RemoteProfile {
    pub id: String,
    pub device_id: String,
    pub username: String,
    pub port: u16,
    pub remembered: bool,
}
#[derive(Clone, Serialize, JsonSchema)]
pub enum RemotePhase {
    Connecting,
    Ready,
    Ended,
    Failed,
    Closed,
}
#[derive(Clone, Serialize, JsonSchema)]
pub struct RemoteConnection {
    pub state: RemotePhase,
    pub error: Option<UiError>,
}
impl From<core::SshConnectionState> for RemoteConnection {
    fn from(value: core::SshConnectionState) -> Self {
        use core::SshConnectionState as S;
        let (state, error) = match value {
            S::Connecting => (RemotePhase::Connecting, None),
            S::Ready => (RemotePhase::Ready, None),
            S::Ended => (RemotePhase::Ended, None),
            S::Closed => (RemotePhase::Closed, None),
            S::Failed(e) => (RemotePhase::Failed, Some(errors::ssh(e))),
        };
        Self { state, error }
    }
}
#[derive(Clone, Serialize, JsonSchema)]
pub enum RemoteKind {
    Directory,
    File,
    Symlink,
    Other,
    Unknown,
}
#[derive(Clone, Serialize, JsonSchema)]
pub struct RemoteEntry {
    pub name: String,
    pub path: String,
    pub kind: RemoteKind,
    pub size: Option<f64>,
    pub modified: Option<u32>,
}
#[derive(Clone, Serialize, JsonSchema)]
pub enum RemoteDirectoryState {
    Loading,
    Partial,
    Complete,
    Stale,
    Error,
}
#[derive(Clone, Serialize, JsonSchema)]
pub struct RemoteDirectory {
    pub path: String,
    pub entries: Vec<RemoteEntry>,
    pub state: RemoteDirectoryState,
    pub error: Option<UiError>,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RemoteWorkspace {
    pub id: String,
    pub profile_id: String,
    pub device_id: String,
    pub username: String,
    pub port: u16,
    pub address: Option<String>,
    pub terminal_id: String,
    pub files_id: String,
    pub terminal: RemoteConnection,
    pub files: RemoteConnection,
    pub initial_directory: Option<String>,
    pub root: Option<String>,
    pub generation: String,
    /// Omitted when unchanged since the preceding frame. Full on connect.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directories: Option<Vec<RemoteDirectory>>,
    pub error: Option<UiError>,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RemoteChallenge {
    pub id: String,
    pub session_id: String,
    pub address: String,
    pub algorithm: String,
    pub fingerprint: String,
}
#[derive(Clone, Serialize, JsonSchema)]
pub enum RemoteTransferState {
    Queued,
    WaitingForConnection,
    Running,
    Finalizing,
    Succeeded,
    Failed,
    Cancelled,
    Unconfirmed,
}
#[derive(Clone, Serialize, JsonSchema)]
pub enum RemoteDirection {
    Upload,
    Download,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RemoteTransfer {
    pub id: String,
    pub session_id: String,
    pub device_id: String,
    pub direction: RemoteDirection,
    pub source: String,
    pub destination: String,
    pub state: RemoteTransferState,
    pub transferred: f64,
    pub total: Option<f64>,
    pub target: Option<String>,
    pub temporary_path: Option<String>,
    pub error: Option<UiError>,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSnapshot {
    pub revision: String,
    pub profiles: Vec<RemoteProfile>,
    pub workspaces: Vec<RemoteWorkspace>,
    pub challenges: Vec<RemoteChallenge>,
    pub transfers: Vec<RemoteTransfer>,
    pub download_directory: Option<String>,
}
impl From<core::RemoteSnapshot> for RemoteSnapshot {
    fn from(snapshot: core::RemoteSnapshot) -> Self {
        Self::project(snapshot, None)
    }
}
impl RemoteSnapshot {
    pub fn project(
        snapshot: core::RemoteSnapshot,
        previous: Option<&core::RemoteSnapshot>,
    ) -> Self {
        Self {
            revision: snapshot.revision.to_string(),
            download_directory: snapshot
                .download_directory
                .map(|p| p.to_string_lossy().into_owned()),
            profiles: snapshot
                .profiles
                .into_iter()
                .map(|p| {
                    let remembered = p.remembered();
                    RemoteProfile {
                        id: p.id,
                        device_id: p.device_id,
                        username: p.username,
                        port: p.port,
                        remembered,
                    }
                })
                .collect(),
            challenges: snapshot
                .challenges
                .into_iter()
                .map(|c| RemoteChallenge {
                    id: c.id,
                    session_id: c.session_id,
                    address: c.address,
                    algorithm: c.key.algorithm,
                    fingerprint: c.key.fingerprint,
                })
                .collect(),
            workspaces: snapshot
                .workspaces
                .into_iter()
                .map(|w| {
                    let unchanged = previous.is_some_and(|s| {
                        s.workspaces.iter().any(|old| {
                            old.id == w.id
                                && old.files_id == w.files_id
                                && std::sync::Arc::ptr_eq(&old.directories, &w.directories)
                        })
                    });
                    RemoteWorkspace {
                        id: w.id,
                        profile_id: w.profile_id,
                        device_id: w.device_id,
                        username: w.username,
                        port: w.port,
                        address: w.address,
                        terminal_id: w.terminal_id,
                        files_id: w.files_id,
                        terminal: w.terminal.into(),
                        files: w.files.into(),
                        initial_directory: w.initial_directory.map(|p| p.as_str().into()),
                        root: w.root.map(|p| p.as_str().into()),
                        generation: w.generation.to_string(),
                        error: w.error.map(|_| errors::ui("sshCredentials")),
                        directories: (!unchanged).then(|| {
                            w.directories
                                .iter()
                                .cloned()
                                .map(|d| RemoteDirectory {
                                    path: d.path.as_str().into(),
                                    state: match d.state {
                                        core::DirectoryState::Loading => {
                                            RemoteDirectoryState::Loading
                                        }
                                        core::DirectoryState::Partial => {
                                            RemoteDirectoryState::Partial
                                        }
                                        core::DirectoryState::Complete => {
                                            RemoteDirectoryState::Complete
                                        }
                                        core::DirectoryState::Stale => RemoteDirectoryState::Stale,
                                        core::DirectoryState::Error => RemoteDirectoryState::Error,
                                    },
                                    error: d.error.map(errors::ssh),
                                    entries: d
                                        .entries
                                        .into_iter()
                                        .map(|e| RemoteEntry {
                                            name: e.name,
                                            path: e.path.as_str().into(),
                                            size: e.size.map(|n| n as f64),
                                            modified: e.modified,
                                            kind: match e.kind {
                                                core::RemoteFileKind::Directory => {
                                                    RemoteKind::Directory
                                                }
                                                core::RemoteFileKind::File => RemoteKind::File,
                                                core::RemoteFileKind::Symlink => {
                                                    RemoteKind::Symlink
                                                }
                                                core::RemoteFileKind::Other => RemoteKind::Other,
                                                core::RemoteFileKind::Unknown => {
                                                    RemoteKind::Unknown
                                                }
                                            },
                                        })
                                        .collect(),
                                })
                                .collect()
                        }),
                    }
                })
                .collect(),
            transfers: snapshot
                .transfers
                .into_iter()
                .map(|t| {
                    let (direction, source, destination) = match t.spec {
                        core::RemoteTransferSpec::Upload {
                            local,
                            remote_directory,
                        } => (
                            RemoteDirection::Upload,
                            local.to_string_lossy().into_owned(),
                            remote_directory.as_str().into(),
                        ),
                        core::RemoteTransferSpec::Download {
                            remote,
                            local_directory,
                        } => (
                            RemoteDirection::Download,
                            remote.as_str().into(),
                            local_directory.to_string_lossy().into_owned(),
                        ),
                    };
                    RemoteTransfer {
                        id: t.id,
                        session_id: t.session_id,
                        device_id: t.device_id,
                        direction,
                        source,
                        destination,
                        transferred: t.transferred as f64,
                        total: t.total.map(|n| n as f64),
                        target: t.target,
                        temporary_path: t.temporary_path,
                        error: t.error.map(errors::ssh),
                        state: match t.state {
                            core::RemoteTransferState::Queued => RemoteTransferState::Queued,
                            core::RemoteTransferState::WaitingForConnection => {
                                RemoteTransferState::WaitingForConnection
                            }
                            core::RemoteTransferState::Running => RemoteTransferState::Running,
                            core::RemoteTransferState::Finalizing => {
                                RemoteTransferState::Finalizing
                            }
                            core::RemoteTransferState::Succeeded => RemoteTransferState::Succeeded,
                            core::RemoteTransferState::Failed => RemoteTransferState::Failed,
                            core::RemoteTransferState::Cancelled => RemoteTransferState::Cancelled,
                            core::RemoteTransferState::Unconfirmed => {
                                RemoteTransferState::Unconfirmed
                            }
                        },
                    }
                })
                .collect(),
        }
    }
}
