//! Stable WebView value contracts. No executable service or platform handles.
use crate::ipc::errors::UiError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
macro_rules! core_enum {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(Clone, Debug, Serialize, JsonSchema)]
        pub enum $name { $($variant),+ }
    };
}
mod connection;
pub use connection::*;
mod desktop;
pub use desktop::*;
mod settings;
pub use settings::*;
mod devices;
pub use devices::*;
mod clipboard;
pub use clipboard::*;
mod history;
pub use history::*;
mod transfers;
pub use transfers::*;
