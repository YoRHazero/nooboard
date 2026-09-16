//! PAKE-derived, directional AEAD channels. No certificate is trusted before confirmation.
use super::{Error, Result};
use chacha20poly1305::{
    ChaCha20Poly1305, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::Zeroizing;

pub struct Cipher {
    cipher: ChaCha20Poly1305,
    counter: u64,
}
impl Cipher {
    fn new(key: &[u8; 32]) -> Self {
        Self {
            cipher: ChaCha20Poly1305::new(key.into()),
            counter: 0,
        }
    }
    fn nonce(&mut self) -> Result<[u8; 12]> {
        let mut nonce = [0; 12];
        nonce[4..].copy_from_slice(&self.counter.to_be_bytes());
        self.counter = self.counter.checked_add(1).ok_or(Error::Protocol)?;
        Ok(nonce)
    }
    pub fn seal(&mut self, text: &[u8]) -> Result<Vec<u8>> {
        let nonce = self.nonce()?;
        self.cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: text,
                    aad: b"nooboard-pairing-v1",
                },
            )
            .map_err(|_| Error::Protocol)
    }
    pub fn open(&mut self, bytes: &[u8]) -> Result<Vec<u8>> {
        let nonce = self.nonce()?;
        self.cipher
            .decrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: bytes,
                    aad: b"nooboard-pairing-v1",
                },
            )
            .map_err(|_| Error::Code)
    }
}
pub fn channels(key: Vec<u8>, client: bool) -> Result<(Cipher, Cipher)> {
    let key = Zeroizing::new(key);
    let mut keys = Zeroizing::new([0; 64]);
    Hkdf::<Sha256>::new(Some(b"nooboard-pairing-v1"), &key)
        .expand(b"client-to-server|server-to-client", keys.as_mut())
        .map_err(|_| Error::Protocol)?;
    let a = Cipher::new(keys[..32].try_into().map_err(|_| Error::Protocol)?);
    let b = Cipher::new(keys[32..].try_into().map_err(|_| Error::Protocol)?);
    Ok(if client { (a, b) } else { (b, a) })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn directional_channels_reject_reflection_replay_tampering_and_other_sessions() {
        let (mut a, mut a_receive) = channels(vec![7; 32], true).unwrap();
        let (mut b, mut b_receive) = channels(vec![7; 32], false).unwrap();
        let message = a.seal(b"proof").unwrap();
        assert_eq!(b_receive.open(&message).unwrap(), b"proof");
        assert!(b_receive.open(&message).is_err());
        assert!(a_receive.open(&message).is_err());
        let (_, mut wrong) = channels(vec![8; 32], false).unwrap();
        assert!(wrong.open(&message).is_err());
        let (_, mut fresh) = channels(vec![7; 32], false).unwrap();
        let mut tampered = message;
        tampered[0] ^= 1;
        assert!(fresh.open(&tampered).is_err());
        let reply = b.seal(b"reply").unwrap();
        let (_, mut fresh) = channels(vec![7; 32], true).unwrap();
        assert_eq!(fresh.open(&reply).unwrap(), b"reply");
    }
}
