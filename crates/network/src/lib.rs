#![doc = include_str!("../README.md")]
mod api;
mod connections;
mod discovery;
mod error;
mod event;
mod local;
mod options;
mod pairing;
mod peers;
mod random;
mod runtime;
mod transfer;

pub use api::*;

#[cfg(test)]
extern crate self as nooboard_network;
#[cfg(test)]
#[path = "../tests/support/identity.rs"]
mod test_identity;
