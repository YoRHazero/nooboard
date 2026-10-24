use crate::contracts::{PortFuture, terminal::SshAddressSource};
pub(crate) trait DeviceAddressPort: Send + Sync {
    fn resolve(&self, address: SshAddressSource, port: u16)
    -> PortFuture<'_, std::net::SocketAddr>;
}
