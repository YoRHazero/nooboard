//! Standalone terminal, managed command, SSH forwarding and SFTP service.
//! Callers provide address resolution, authentication and host trust policy.
mod api;
mod auto_input;
mod backend;
mod model;
mod options;
mod output;
mod runtime;
mod secret;
mod subscription;
pub use api::*;
