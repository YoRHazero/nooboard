use crate::HostKey;

#[derive(Clone, Debug)]
pub enum Event {
    /// Reply only after the application has persisted explicit user approval.
    HostKeyRequired {
        request_id: String,
        session_id: String,
        address: std::net::SocketAddr,
        key: HostKey,
    },
}
