use std::collections::{BTreeMap, HashMap, HashSet};
use std::net::SocketAddr;

use tokio::sync::{mpsc, oneshot};

use crate::errors::{NetworkError, NetworkResult};
use crate::session::actor::SessionCommand;
use crate::{
    ConnectionMode, SendFilesRequest, SendTextRequest, SessionId, SessionInfo, SessionTarget,
    TransferTicket,
};

#[derive(Debug)]
struct SessionRecord {
    info: SessionInfo,
    command_tx: mpsc::Sender<SessionCommand>,
}

#[derive(Debug, Default)]
pub(crate) struct SessionStore {
    sessions: BTreeMap<SessionId, SessionRecord>,
    next_transfer_id_by_session: HashMap<SessionId, u32>,
}

impl SessionStore {
    pub(crate) fn list_sorted(&self) -> Vec<SessionInfo> {
        let mut sessions: Vec<_> = self.sessions.values().map(|record| record.info.clone()).collect();
        sessions.sort_by_key(|session| (session.connected_at_ms, session.id));
        sessions
    }

    pub(crate) fn connected_peer_noob_ids(&self) -> HashSet<String> {
        self.sessions
            .values()
            .map(|record| record.info.peer_noob_id.clone())
            .collect()
    }

    pub(crate) fn remove(&mut self, id: SessionId) -> Option<SessionInfo> {
        self.next_transfer_id_by_session.remove(&id);
        self.sessions.remove(&id).map(|record| record.info)
    }

    pub(crate) fn remove_lan_sessions(&mut self) -> Vec<SessionInfo> {
        let remove_ids: Vec<_> = self
            .sessions
            .iter()
            .filter_map(|(session_id, record)| {
                (record.info.mode == ConnectionMode::Lan).then_some(*session_id)
            })
            .collect();
        let mut removed = Vec::with_capacity(remove_ids.len());
        for session_id in remove_ids {
            self.next_transfer_id_by_session.remove(&session_id);
            if let Some(record) = self.sessions.remove(&session_id) {
                let _ = try_send_session_command(
                    &record.command_tx,
                    SessionCommand::Shutdown,
                    session_id,
                    "disconnect lan session",
                );
                removed.push(record.info);
            }
        }
        removed
    }

    pub(crate) fn validate_target(&self, target: &SessionTarget) -> NetworkResult<()> {
        match target {
            SessionTarget::AllConnected => Ok(()),
            SessionTarget::Sessions(ids) => {
                for id in ids {
                    if !self.sessions.contains_key(id) {
                        return Err(NetworkError::SessionNotFound(*id));
                    }
                }
                Ok(())
            }
        }
    }

    pub(crate) fn find_direct_by_remote_addr(&self, remote_addr: SocketAddr) -> Option<SessionId> {
        self.sessions.iter().find_map(|(id, record)| {
            if record.info.mode == ConnectionMode::Direct && record.info.remote_addr == remote_addr {
                Some(*id)
            } else {
                None
            }
        })
    }

    pub(crate) fn find_by_peer_noob_id(&self, peer_noob_id: &str) -> Option<SessionId> {
        self.sessions.iter().find_map(|(id, record)| {
            (record.info.peer_noob_id == peer_noob_id).then_some(*id)
        })
    }

    pub(crate) fn insert(&mut self, info: SessionInfo, command_tx: mpsc::Sender<SessionCommand>) {
        self.next_transfer_id_by_session.entry(info.id).or_insert(1);
        self.sessions.insert(info.id, SessionRecord { info, command_tx });
    }

    pub(crate) fn send_text(&self, request: &SendTextRequest) -> NetworkResult<()> {
        match &request.target {
            SessionTarget::AllConnected => {
                for record in self.sessions.values() {
                    try_send_session_command(
                        &record.command_tx,
                        SessionCommand::SendText {
                            event_id: request.event_id.clone(),
                            content: request.content.clone(),
                        },
                        record.info.id,
                        "send text",
                    )?;
                }
                Ok(())
            }
            SessionTarget::Sessions(ids) => {
                for id in ids {
                    let record = self
                        .sessions
                        .get(id)
                        .ok_or(NetworkError::SessionNotFound(*id))?;
                    try_send_session_command(
                        &record.command_tx,
                        SessionCommand::SendText {
                            event_id: request.event_id.clone(),
                            content: request.content.clone(),
                        },
                        record.info.id,
                        "send text",
                    )?;
                }
                Ok(())
            }
        }
    }

    pub(crate) fn send_files(
        &mut self,
        request: &SendFilesRequest,
    ) -> NetworkResult<Vec<TransferTicket>> {
        let target_ids = self.resolve_target_ids(&request.target)?;
        let mut tickets = Vec::new();

        for session_id in target_ids {
            let command_tx = self
                .sessions
                .get(&session_id)
                .map(|record| record.command_tx.clone())
                .ok_or(NetworkError::SessionNotFound(session_id))?;

            for path in &request.files {
                let transfer_id = self.next_transfer_id(session_id);
                try_send_session_command(
                    &command_tx,
                    SessionCommand::SendFile {
                        transfer_id,
                        path: path.clone(),
                    },
                    session_id,
                    "send file",
                )?;
                tickets.push(TransferTicket {
                    session_id,
                    raw_id: transfer_id,
                });
            }
        }

        Ok(tickets)
    }

    pub(crate) fn decide_incoming_transfer(
        &self,
        ticket: TransferTicket,
        accept: bool,
        reason: Option<String>,
    ) -> NetworkResult<()> {
        let record = self
            .sessions
            .get(&ticket.session_id)
            .ok_or(NetworkError::SessionNotFound(ticket.session_id))?;
        try_send_session_command(
            &record.command_tx,
            SessionCommand::FileDecision {
                transfer_id: ticket.raw_id,
                accept,
                reason,
            },
            ticket.session_id,
            "decide incoming transfer",
        )
    }

    pub(crate) async fn cancel_transfer(&self, ticket: TransferTicket) -> NetworkResult<()> {
        let record = self
            .sessions
            .get(&ticket.session_id)
            .ok_or(NetworkError::SessionNotFound(ticket.session_id))?;
        let (reply_tx, reply_rx) = oneshot::channel();
        try_send_session_command(
            &record.command_tx,
            SessionCommand::CancelTransfer {
                transfer_id: ticket.raw_id,
                reply: reply_tx,
            },
            ticket.session_id,
            "cancel transfer",
        )?;
        reply_rx
            .await
            .map_err(|_| NetworkError::ChannelClosed)?
            .map_err(|error| NetworkError::Internal(error.to_string()))
    }

    pub(crate) fn disconnect(&self, session_id: SessionId) -> NetworkResult<()> {
        let record = self
            .sessions
            .get(&session_id)
            .ok_or(NetworkError::SessionNotFound(session_id))?;
        try_send_session_command(
            &record.command_tx,
            SessionCommand::Shutdown,
            session_id,
            "disconnect session",
        )
    }

    pub(crate) fn shutdown_all(&self) {
        for (session_id, record) in &self.sessions {
            let _ = try_send_session_command(
                &record.command_tx,
                SessionCommand::Shutdown,
                *session_id,
                "shutdown runtime",
            );
        }
    }

    fn resolve_target_ids(&self, target: &SessionTarget) -> NetworkResult<Vec<SessionId>> {
        match target {
            SessionTarget::AllConnected => Ok(self.sessions.keys().copied().collect()),
            SessionTarget::Sessions(ids) => {
                self.validate_target(target)?;
                Ok(ids.clone())
            }
        }
    }

    fn next_transfer_id(&mut self, session_id: SessionId) -> u32 {
        let entry = self.next_transfer_id_by_session.entry(session_id).or_insert(1);
        let next = *entry;
        *entry = entry.wrapping_add(1);
        next
    }
}

fn try_send_session_command(
    sender: &mpsc::Sender<SessionCommand>,
    command: SessionCommand,
    session_id: SessionId,
    op: &str,
) -> NetworkResult<()> {
    sender.try_send(command).map_err(|error| match error {
        mpsc::error::TrySendError::Full(_) => {
            NetworkError::Internal(format!("session {session_id} queue is full while {op}"))
        }
        mpsc::error::TrySendError::Closed(_) => {
            NetworkError::Internal(format!("session {session_id} queue is closed while {op}"))
        }
    })
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;

    use super::*;

    #[test]
    fn list_sorted_orders_by_connected_time_then_id() {
        let mut store = SessionStore::default();
        let (tx1, _) = mpsc::channel(1);
        let (tx2, _) = mpsc::channel(1);
        let later = SessionInfo {
            id: SessionId::new(),
            mode: ConnectionMode::Direct,
            peer_noob_id: "b".to_string(),
            peer_device_id: "device-b".to_string(),
            remote_addr: "127.0.0.1:1001".parse().expect("addr"),
            local_bind_addr: None,
            outbound: true,
            connected_at_ms: 2,
        };
        let earlier = SessionInfo {
            id: SessionId::new(),
            mode: ConnectionMode::Lan,
            peer_noob_id: "a".to_string(),
            peer_device_id: "device-a".to_string(),
            remote_addr: "127.0.0.1:1000".parse().expect("addr"),
            local_bind_addr: None,
            outbound: false,
            connected_at_ms: 1,
        };
        store.insert(later, tx1);
        store.insert(earlier, tx2);

        let sessions = store.list_sorted();
        assert_eq!(sessions[0].connected_at_ms, 1);
        assert_eq!(sessions[1].connected_at_ms, 2);
    }
}
