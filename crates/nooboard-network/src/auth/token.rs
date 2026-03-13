use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub(crate) fn compute_auth_hash(token: &str, nonce: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(token.as_bytes()).expect("HMAC supports arbitrary token length");
    mac.update(nonce.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}
