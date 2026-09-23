mod credentials;
pub(crate) mod material;
mod model;
use crate::error::{Failure, InternalResult as Result};
use material::Identity;
pub use model::{IdentityOptions, PublicIdentity, TrustedPeer};
use std::sync::Arc;

pub(crate) fn valid_device_name(name: &str) -> bool {
    !name.trim().is_empty() && name.chars().count() <= 80 && !name.chars().any(char::is_control)
}
pub(crate) async fn load(options: IdentityOptions) -> Result<Arc<Identity>> {
    tokio::task::spawn_blocking(move || match options {
        IdentityOptions::Ephemeral => Identity::generate(),
        IdentityOptions::System { profile } => credentials::load(&profile),
    })
    .await
    .map_err(|_| Failure::Internal)?
    .map(Arc::new)
}
