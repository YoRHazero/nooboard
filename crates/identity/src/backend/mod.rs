mod credentials;
mod crypto;
mod material;
use crate::{IdentityConfig, IdentityOptions, Result};
pub(crate) use material::Material;
pub(crate) fn load(config: IdentityConfig, options: &IdentityOptions) -> Result<Material> {
    match config {
        IdentityConfig::Ephemeral => Material::generate(),
        IdentityConfig::System { profile } => credentials::load(&profile, options),
    }
}
