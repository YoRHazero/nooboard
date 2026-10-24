pub(crate) mod certificate;
mod model;
pub use model::{PublicIdentity, TrustedPeer};
pub(crate) fn valid_device_name(name: &str) -> bool {
    !name.trim().is_empty() && name.chars().count() <= 80 && !name.chars().any(char::is_control)
}
