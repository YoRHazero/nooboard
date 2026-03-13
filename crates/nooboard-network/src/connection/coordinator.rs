use std::collections::HashMap;
use std::net::SocketAddr;

use tokio::task::JoinHandle;

use crate::ConnectionMode;

struct OutboundAttempt {
    mode: ConnectionMode,
    task: JoinHandle<()>,
}

#[derive(Default)]
pub(crate) struct ConnectionCoordinator {
    outbound_attempts: HashMap<SocketAddr, OutboundAttempt>,
}

pub(crate) enum BeginAttempt {
    Granted,
    Skipped,
}

impl ConnectionCoordinator {
    pub(crate) fn begin(
        &mut self,
        mode: ConnectionMode,
        addr: SocketAddr,
        task: JoinHandle<()>,
    ) -> BeginAttempt {
        match self.outbound_attempts.remove(&addr) {
            Some(existing) if existing.mode == ConnectionMode::Lan && mode == ConnectionMode::Direct => {
                existing.task.abort();
                self.outbound_attempts
                    .insert(addr, OutboundAttempt { mode, task });
                BeginAttempt::Granted
            }
            Some(existing) => {
                self.outbound_attempts.insert(addr, existing);
                task.abort();
                BeginAttempt::Skipped
            }
            None => {
                self.outbound_attempts
                    .insert(addr, OutboundAttempt { mode, task });
                BeginAttempt::Granted
            }
        }
    }

    pub(crate) fn finish(&mut self, mode: ConnectionMode, addr: SocketAddr) {
        let should_remove = self
            .outbound_attempts
            .get(&addr)
            .is_some_and(|attempt| attempt.mode == mode);
        if should_remove {
            self.outbound_attempts.remove(&addr);
        }
    }

    pub(crate) fn abort_all(&mut self) {
        for (_, attempt) in self.outbound_attempts.drain() {
            attempt.task.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio::task::JoinHandle;

    use super::*;

    fn task() -> JoinHandle<()> {
        tokio::spawn(async {})
    }

    #[tokio::test]
    async fn direct_supersedes_lan_attempt_for_same_addr() {
        let mut coordinator = ConnectionCoordinator::default();
        let addr: SocketAddr = "127.0.0.1:17890".parse().expect("addr");

        assert!(matches!(
            coordinator.begin(ConnectionMode::Lan, addr, task()),
            BeginAttempt::Granted
        ));
        assert!(matches!(
            coordinator.begin(ConnectionMode::Direct, addr, task()),
            BeginAttempt::Granted
        ));
    }
}
