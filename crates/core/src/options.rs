use std::path::PathBuf;
/// Host-specific startup choices. Database selection never enters business requests.
pub struct Options {
    pub storage: nooboard_storage::BackendConfig,
    pub profile: String,
    /// Fills an unset receive directory after loading or migrating configuration.
    /// An explicitly configured directory takes precedence; the fallback is saved.
    pub default_receive_directory: Option<PathBuf>,
}
