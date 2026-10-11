use crate::RemotePath;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum FileKind {
    Directory,
    File,
    Symlink,
    Other,
    Unknown,
}
#[derive(Clone, Debug, Serialize)]
pub struct DirectoryEntry {
    pub name: String,
    pub path: RemotePath,
    pub kind: FileKind,
    pub size: Option<u64>,
    pub modified: Option<u32>,
}
#[derive(Clone, Debug, Serialize)]
pub struct DirectoryBatch {
    pub session_id: String,
    pub query_id: String,
    pub path: RemotePath,
    pub entries: Vec<DirectoryEntry>,
    pub cursor: Option<String>,
    pub complete: bool,
}
