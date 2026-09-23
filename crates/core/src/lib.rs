//! Application policies and workflows. Only this crate coordinates the three services.
#![doc = include_str!("../README.md")]
mod api;
mod configuration;
mod devices;
#[cfg(feature = "diagnostics")]
#[path = "diagnostics.rs"]
mod diagnostic_support;
mod error;
mod history;
mod options;
mod ports;
mod runtime;
mod snapshot;
mod sync;
pub use api::*;
#[cfg(test)]
mod tests;
