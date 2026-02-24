use std::collections::HashMap;
use std::time::{Duration, Instant};

use hmac::{Hmac, Mac};
use sha2::Sha256;
use tokio::sync::Mutex;
use uuid::Uuid;

pub type SocketId = u64;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthCheck {
    Accepted,
    Rejected,
    Timeout,
    Missing,
}

#[derive(Debug, Clone)]
struct PendingChallenge {
    nonce: String,
    expires_at: Instant,
}

#[derive(Debug, Default)]
pub struct ChallengeRegistry {
    inner: Mutex<HashMap<SocketId, PendingChallenge>>,
}

impl ChallengeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn issue_challenge(&self, socket_id: SocketId, timeout: Duration) -> String {
        let nonce = Uuid::now_v7().to_string();
        let pending = PendingChallenge {
            nonce: nonce.clone(),
            expires_at: Instant::now() + timeout,
        };
        self.inner.lock().await.insert(socket_id, pending);
        nonce
    }

    pub async fn verify_response(
        &self,
        socket_id: SocketId,
        token: &str,
        response_hash: &str,
    ) -> AuthCheck {
        let pending = self.inner.lock().await.remove(&socket_id);
        let Some(pending) = pending else {
            return AuthCheck::Missing;
        };

        if Instant::now() > pending.expires_at {
            return AuthCheck::Timeout;
        }

        let expected = compute_auth_hash(token, &pending.nonce);
        if expected == response_hash {
            AuthCheck::Accepted
        } else {
            AuthCheck::Rejected
        }
    }

    pub async fn clear(&self, socket_id: SocketId) {
        self.inner.lock().await.remove(&socket_id);
    }

    pub async fn prune_expired(&self) {
        let now = Instant::now();
        self.inner
            .lock()
            .await
            .retain(|_, entry| entry.expires_at > now);
    }

    pub async fn pending_count(&self) -> usize {
        self.inner.lock().await.len()
    }
}

pub fn compute_auth_hash(token: &str, nonce: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(token.as_bytes()).expect("HMAC supports arbitrary token length");
    mac.update(nonce.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn challenge_is_released_after_success() {
        let registry = ChallengeRegistry::new();
        let nonce = registry.issue_challenge(1, Duration::from_secs(1)).await;
        let hash = compute_auth_hash("token", &nonce);
        let result = registry.verify_response(1, "token", &hash).await;

        assert_eq!(result, AuthCheck::Accepted);
        assert_eq!(registry.pending_count().await, 0);
    }

    #[tokio::test]
    async fn challenge_is_released_after_failure() {
        let registry = ChallengeRegistry::new();
        let _ = registry.issue_challenge(2, Duration::from_secs(1)).await;
        let result = registry.verify_response(2, "token", "invalid").await;

        assert_eq!(result, AuthCheck::Rejected);
        assert_eq!(registry.pending_count().await, 0);
    }

    #[tokio::test]
    async fn challenge_is_released_after_timeout() {
        let registry = ChallengeRegistry::new();
        let nonce = registry.issue_challenge(3, Duration::from_millis(5)).await;
        let hash = compute_auth_hash("token", &nonce);

        tokio::time::sleep(Duration::from_millis(10)).await;
        let result = registry.verify_response(3, "token", &hash).await;

        assert_eq!(result, AuthCheck::Timeout);
        assert_eq!(registry.pending_count().await, 0);
    }
}
