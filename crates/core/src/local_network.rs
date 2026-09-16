//! Local connection details derived from live listeners, never persisted as identity.
pub use nooboard_network::local::LocalAddress;
use serde::Serialize;

#[derive(Clone, Default, PartialEq, Eq, Serialize)]
pub struct LocalNetwork {
    pub sync_port: u16,
    pub pairing_port: u16,
    pub addresses: Vec<LocalAddress>,
    pub error: Option<String>,
}
