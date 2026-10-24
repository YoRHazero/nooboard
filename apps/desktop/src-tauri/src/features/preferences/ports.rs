use super::domain::Preferences;
use crate::error::Result;
pub trait PreferenceRepository: Send + Sync {
    fn available(&self) -> bool {
        true
    }
    fn load(&self) -> Result<Preferences>;
    fn save(&self, value: &Preferences) -> Result<()>;
}
pub trait SystemLocale: Send + Sync {
    fn locale(&self) -> String;
}
