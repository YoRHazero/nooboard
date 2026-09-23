mod history;
mod settings;
pub use history::*;
pub use settings::*;

pub const MAX_TEXT_BYTES: usize = 1024 * 1024;
pub(crate) fn text(value: &str, limit: usize, argument: &'static str) -> crate::Result<()> {
    if value.len() > limit || value.contains('\0') {
        Err(crate::Error::invalid(argument))
    } else {
        Ok(())
    }
}
