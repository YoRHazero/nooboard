#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionStatus {
    pub peer: String,
    pub device_name: String,
    pub connected: bool,
    pub accepting: bool,
    /// The remote device's sync pause, independent of its receiving permission.
    pub paused: bool,
    /// Runtime-only receipt identifier; receiving it blocks automatic dialing.
    pub unpaired_notice: Option<u64>,
    pub(crate) generation: u64,
}
