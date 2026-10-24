//! Application policies and composition. Services remain independent.
#![doc = include_str!("../README.md")]
mod api;
mod app;
mod clock;
mod configuration;
mod contracts;
#[cfg(feature = "diagnostics")]
#[path = "diagnostics.rs"]
mod diagnostic_support;
mod error;
mod features;
mod message;
pub use api::*;
#[cfg(test)]
mod tests;
