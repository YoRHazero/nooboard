mod defaults;
mod io;
mod mapping;
mod noob_id;
mod schema;
mod validate;

pub use defaults::{APP_CONFIG_VERSION, DEFAULT_MAX_TEXT_BYTES, DEFAULT_RECENT_EVENT_LOOKUP_LIMIT};
pub use schema::AppConfig;

#[cfg(test)]
mod tests;
