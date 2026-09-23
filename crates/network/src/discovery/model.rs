use serde::Serialize;
use std::net::{IpAddr, SocketAddr};

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct NearbyDevice {
    pub key: String,
    pub noob_id: String,
    pub device_name: String,
    pub addresses: Vec<String>,
    pub sync_port: u16,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LocalAddress {
    pub interface: String,
    pub ip: IpAddr,
    pub pairing_address: SocketAddr,
}
