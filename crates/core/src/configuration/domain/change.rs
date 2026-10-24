use crate::{PeerSettings, Settings, contracts::network::TrustedPeer};
pub(crate) enum Change {
    Settings(Settings),
    SavePeer(TrustedPeer),
    RemovePeer(String),
    ConfigurePeer(String, PeerSettings),
    PeerNames(Vec<(String, String)>),
    Targets(Vec<String>),
}
