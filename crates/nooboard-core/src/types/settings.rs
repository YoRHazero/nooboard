use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSettings {
    pub connection: ConnectionSettings,
    pub network: NetworkSettings,
    pub storage: StorageSettings,
    pub clipboard: ClipboardSettings,
    pub transfers: TransferSettings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionSettings {
    pub device_id: String,
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkSettings {
    pub listen_port: u16,
    pub lan_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageSettings {
    pub db_root: PathBuf,
    pub history_window_days: u32,
    pub dedup_window_days: u32,
    pub max_text_bytes: usize,
    pub gc_batch_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageSettingsInput {
    pub history_window_days: u32,
    pub dedup_window_days: u32,
    pub max_text_bytes: usize,
    pub gc_batch_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardSettings {
    pub local_capture_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferSettings {
    pub download_dir: PathBuf,
}
