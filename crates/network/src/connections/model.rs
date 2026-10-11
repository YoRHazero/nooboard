#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionStatus {
    pub peer: String,
    pub device_name: String,
    pub connected: bool,
    /// Remote socket endpoint of the live connection, cleared on disconnect.
    /// For incoming connections the port may be an ephemeral client port.
    pub peer_address: Option<std::net::SocketAddr>,
    pub accepting: bool,
    /// The remote device's sync pause, independent of its receiving permission.
    pub paused: bool,
    /// Runtime-only receipt identifier; receiving it blocks automatic dialing.
    pub unpaired_notice: Option<u64>,
    pub(crate) generation: u64,
}
