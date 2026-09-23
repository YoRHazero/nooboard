#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionStatus {
    pub peer: String,
    pub device_name: String,
    pub connected: bool,
    pub accepting: bool,
    pub(crate) generation: u64,
}
