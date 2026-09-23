use crate::{
    error::{Failure, InternalResult as Result},
    identity::{IdentityOptions, TrustedPeer, valid_device_name},
};
use std::{net::SocketAddr, time::Duration};

#[derive(Clone, Debug)]
pub struct Options {
    pub identity: IdentityOptions,
    pub device_name: String,
    pub listen: SocketAddr,
    pub pairing_listen: SocketAddr,
    pub discovery: bool,
    pub trusted_peers: Vec<TrustedPeer>,
    pub queue_capacity: usize,
    pub event_capacity: usize,
    pub max_transfers: usize,
    pub operation_timeout: Duration,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            identity: IdentityOptions::System {
                profile: "default".into(),
            },
            device_name: "Nooboard".into(),
            listen: "0.0.0.0:24816".parse().unwrap(),
            pairing_listen: "0.0.0.0:24817".parse().unwrap(),
            discovery: true,
            trusted_peers: vec![],
            queue_capacity: 32,
            event_capacity: 32,
            max_transfers: 32,
            operation_timeout: Duration::from_secs(60),
        }
    }
}
impl Options {
    pub(crate) fn validate(&self) -> Result<()> {
        if !valid_device_name(&self.device_name)
            || !(1..=256).contains(&self.queue_capacity)
            || !(1..=256).contains(&self.event_capacity)
            || !(1..=128).contains(&self.max_transfers)
            || self.trusted_peers.len() > 64
            || self.operation_timeout < Duration::from_millis(100)
            || self.operation_timeout > Duration::from_secs(300)
        {
            return Err(Failure::InvalidArgument("network options"));
        }
        if let IdentityOptions::System { profile } = &self.identity
            && (profile.is_empty() || profile.len() > 256 || profile.chars().any(char::is_control))
        {
            return Err(Failure::InvalidArgument("identity profile"));
        }
        Ok(())
    }
}
