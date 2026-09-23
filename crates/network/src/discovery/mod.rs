mod interfaces;
mod mdns;
mod model;
pub(crate) mod runtime;
pub(crate) use interfaces::pairing_addresses;
pub use model::{LocalAddress, NearbyDevice};
