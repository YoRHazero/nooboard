use super::Runtime;
use crate::LocalNetwork;
use std::net::SocketAddr;

impl Runtime {
    pub(super) fn refresh_local_network(&mut self) -> bool {
        let pairing: SocketAddr = self
            .onboarding
            .endpoint
            .address
            .parse()
            .expect("bound address");
        let sync: SocketAddr = self.listener.address.parse().expect("bound address");
        let (addresses, error) = match nooboard_network::local::pairing_addresses(pairing, sync) {
            Ok(addresses) => (addresses, None),
            Err(_) => (Vec::new(), Some("暂时无法读取本机 IP，将自动重试。".into())),
        };
        let network = LocalNetwork {
            sync_port: sync.port(),
            pairing_port: pairing.port(),
            addresses,
            error,
        };
        if network == self.view.snapshot.local_network {
            return false;
        }
        self.view.snapshot.local_network = network;
        true
    }
}
