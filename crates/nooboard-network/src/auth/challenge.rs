use std::collections::HashMap;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;
use uuid::Uuid;

pub(crate) type SocketId = u64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AuthCheck {
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
pub(crate) struct ChallengeRegistry {
    inner: Mutex<HashMap<SocketId, PendingChallenge>>,
}

impl ChallengeRegistry {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) async fn issue_challenge(&self, socket_id: SocketId, timeout: Duration) -> String {
        let nonce = Uuid::now_v7().to_string();
        let pending = PendingChallenge {
            nonce: nonce.clone(),
            expires_at: Instant::now() + timeout,
        };
        self.inner.lock().await.insert(socket_id, pending);
        nonce
    }

    pub(crate) async fn verify_response(
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

        let expected = super::compute_auth_hash(token, &pending.nonce);
        if expected == response_hash {
            AuthCheck::Accepted
        } else {
            AuthCheck::Rejected
        }
    }

    pub(crate) async fn clear(&self, socket_id: SocketId) {
        self.inner.lock().await.remove(&socket_id);
    }

    pub(crate) async fn prune_expired(&self) {
        let now = Instant::now();
        self.inner
            .lock()
            .await
            .retain(|_, entry| entry.expires_at > now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn challenge_is_released_after_success() {
        let registry = ChallengeRegistry::new();
        let nonce = registry.issue_challenge(1, Duration::from_secs(1)).await;
        let hash = crate::auth::compute_auth_hash("token", &nonce);
        let result = registry.verify_response(1, "token", &hash).await;

        assert_eq!(result, AuthCheck::Accepted);
    }
}
