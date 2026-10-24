use crate::error::{Failure as Error, InternalResult as Result};
/// A fresh, cryptographically random namespace for message IDs after each restart.
pub fn new_session_id() -> Result<String> {
    let mut bytes = [0_u8; 16];
    rustls::crypto::ring::default_provider()
        .secure_random
        .fill(&mut bytes)
        .map_err(|_| Error::Identity)?;
    Ok(hex::encode(bytes))
}
