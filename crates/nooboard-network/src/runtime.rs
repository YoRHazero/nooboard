use crate::config::NetworkConfig;
use crate::errors::NetworkResult;
use crate::manager::RuntimeManager;
use crate::{
    ConnectDirectOutcome, DirectRequestId, DirectSeedId, DirectSeedInfo, IncomingTransferDecision,
    NetworkSnapshot, NetworkSubscription, PendingDirectRequest, SendFilesRequest, SendTextRequest,
    SessionId, SessionInfo, TransferTicket, UpsertDirectSeedInput,
};

#[derive(Clone)]
pub struct NetworkRuntime {
    manager: RuntimeManager,
}

impl NetworkRuntime {
    pub fn new(config: NetworkConfig) -> NetworkResult<Self> {
        Ok(Self {
            manager: RuntimeManager::new(config)?,
        })
    }

    pub async fn start(&self) -> NetworkResult<()> {
        self.manager.start().await
    }

    pub async fn shutdown(&self) -> NetworkResult<()> {
        self.manager.shutdown().await
    }

    pub async fn snapshot(&self) -> NetworkResult<NetworkSnapshot> {
        self.manager.snapshot().await
    }

    pub fn subscribe(&self) -> NetworkSubscription {
        self.manager.subscribe()
    }

    pub async fn set_lan_enabled(&self, enabled: bool) -> NetworkResult<()> {
        self.manager.set_lan_enabled(enabled).await
    }

    pub async fn list_direct_seeds(&self) -> NetworkResult<Vec<DirectSeedInfo>> {
        self.manager.list_direct_seeds().await
    }

    pub async fn upsert_direct_seed(
        &self,
        input: UpsertDirectSeedInput,
    ) -> NetworkResult<DirectSeedId> {
        self.manager.upsert_direct_seed(input).await
    }

    pub async fn remove_direct_seed(&self, id: DirectSeedId) -> NetworkResult<()> {
        self.manager.remove_direct_seed(id).await
    }

    pub async fn search_direct_seeds(&self, query: &str) -> NetworkResult<Vec<DirectSeedInfo>> {
        self.manager.search_direct_seeds(query).await
    }

    pub async fn connect_direct_seed(
        &self,
        id: DirectSeedId,
    ) -> NetworkResult<ConnectDirectOutcome> {
        self.manager.connect_direct_seed(id).await
    }

    pub async fn list_pending_direct_requests(
        &self,
    ) -> NetworkResult<Vec<PendingDirectRequest>> {
        self.manager.list_pending_direct_requests().await
    }

    pub async fn approve_direct_request(&self, id: DirectRequestId) -> NetworkResult<()> {
        self.manager.approve_direct_request(id).await
    }

    pub async fn reject_direct_request(&self, id: DirectRequestId) -> NetworkResult<()> {
        self.manager.reject_direct_request(id).await
    }

    pub async fn list_sessions(&self) -> NetworkResult<Vec<SessionInfo>> {
        self.manager.list_sessions().await
    }

    pub async fn disconnect_session(&self, id: SessionId) -> NetworkResult<()> {
        self.manager.disconnect_session(id).await
    }

    pub async fn send_text(&self, request: SendTextRequest) -> NetworkResult<()> {
        self.manager.send_text(request).await
    }

    pub async fn send_files(
        &self,
        request: SendFilesRequest,
    ) -> NetworkResult<Vec<TransferTicket>> {
        self.manager.send_files(request).await
    }

    pub async fn decide_incoming_transfer(
        &self,
        decision: IncomingTransferDecision,
    ) -> NetworkResult<()> {
        self.manager.decide_incoming_transfer(decision).await
    }

    pub async fn cancel_transfer(&self, id: TransferTicket) -> NetworkResult<()> {
        self.manager.cancel_transfer(id).await
    }
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
