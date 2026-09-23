use super::{
    material::{Identity, fingerprint, noob_id},
    valid_device_name,
};
use crate::error::{Failure, InternalResult as Result};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Clone, Debug)]
pub enum IdentityOptions {
    System {
        profile: String,
    },
    /// Explicitly temporary identity; never a fallback for a credential failure.
    Ephemeral,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicIdentity {
    pub id: String,
    pub fingerprint: String,
    pub device_name: String,
    pub certificate: Vec<u8>,
}
impl PublicIdentity {
    /// Derive public identity metadata from a certificate. This does not authorize
    /// the device or establish trust; the caller must have a separate trust decision.
    pub fn from_certificate(
        device_name: String,
        certificate: Vec<u8>,
    ) -> std::result::Result<Self, crate::error::Error> {
        if certificate.len() > 8192 || !valid_device_name(&device_name) {
            return Err(Failure::Identity.into());
        }
        let identity = Self {
            id: noob_id(&certificate)?,
            fingerprint: fingerprint(&certificate),
            device_name,
            certificate,
        };
        identity.validate()?;
        Ok(identity)
    }
    pub(crate) fn from_identity(identity: &Identity, device_name: String) -> Result<Self> {
        Ok(Self {
            id: identity.noob_id()?,
            fingerprint: identity.fingerprint(),
            device_name,
            certificate: identity.certificate().to_vec(),
        })
    }
    pub(crate) fn validate(&self) -> Result<()> {
        if !valid_device_name(&self.device_name)
            || self.certificate.len() > 8192
            || self.id != noob_id(&self.certificate)?
            || self.fingerprint != fingerprint(&self.certificate)
        {
            return Err(Failure::Identity);
        }
        Ok(())
    }
}
/// Public trust record for the caller's configuration store. Contains no private key.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrustedPeer {
    pub identity: PublicIdentity,
    pub addresses: Vec<SocketAddr>,
}
impl TrustedPeer {
    pub(crate) fn validate(&self, own_id: &str) -> Result<()> {
        self.identity.validate()?;
        if self.identity.id == own_id
            || self.addresses.len() > 16
            || self
                .addresses
                .iter()
                .any(|a| a.port() == 0 || a.ip().is_unspecified() || a.ip().is_multicast())
        {
            return Err(Failure::InvalidArgument("trusted peer"));
        }
        Ok(())
    }
}
